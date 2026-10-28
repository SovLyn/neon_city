use super::*;

struct Emitter {
    desc: EmitterDesc,
    rng: Rng,
    credit: f32,
    event_left: f32,
    burst_left: f32,
}
// World-space triangles and their cumulative areas are built once, not per tick.
struct Puddle {
    triangles: Vec<([Vec3; 3], f32)>,
    area: f32,
}
impl Puddle {
    fn new(mesh: &MeshData, world: Mat4) -> Option<Self> {
        let mut triangles = Vec::new();
        let mut area = 0.0;
        for indices in mesh.indices.chunks_exact(3) {
            let (Some(a), Some(b), Some(c)) = (
                mesh.vertices.get(indices[0] as usize),
                mesh.vertices.get(indices[1] as usize),
                mesh.vertices.get(indices[2] as usize),
            ) else {
                continue;
            };
            let points = [a, b, c].map(|v| world.transform_point3(Vec3::from_array(v.position)));
            let weight = (points[1] - points[0])
                .cross(points[2] - points[0])
                .length()
                * 0.5;
            if weight > 0.0 && weight.is_finite() && (area + weight).is_finite() {
                area += weight;
                triangles.push((points, area));
            }
        }
        (!triangles.is_empty()).then_some(Self { triangles, area })
    }

    fn sample(&self, rng: &mut Rng) -> Vec3 {
        let target = rng.unit() * self.area;
        let index = self
            .triangles
            .partition_point(|(_, end)| *end <= target)
            .min(self.triangles.len() - 1);
        let [a, b, c] = self.triangles[index].0;
        // Square-root barycentric sampling is uniform over the selected triangle.
        let u = rng.unit().sqrt();
        let v = rng.unit();
        a * (1.0 - u) + b * (u * (1.0 - v)) + c * (u * v)
    }
}
pub(super) struct Weather {
    pub particles: Vec<ParticleGpu>,
    pub ripples: Vec<RippleGpu>,
    emitters: Vec<Emitter>,
    puddles: Vec<Puddle>,
    ripple_rng: Rng,
    ripple_left: f32,
}
impl Weather {
    pub fn new(scene: &SceneCpu, seed: u64) -> Self {
        let mut weather = Self {
            particles: Vec::new(),
            ripples: Vec::new(),
            emitters: scene
                .emitters
                .iter()
                .enumerate()
                .map(|(index, desc)| {
                    // Emitter seed is authoritative; different systems never consume shared entropy.
                    let mut rng = Rng::new(seed ^ desc.seed, 10 + desc.kind as u64, index as u64);
                    let event_left = rng.range(0.0, 5.0);
                    Emitter {
                        desc: desc.clone(),
                        credit: rng.unit(),
                        event_left,
                        burst_left: 0.0,
                        rng,
                    }
                })
                .collect(),
            puddles: scene
                .instances
                .iter()
                .filter(|i| {
                    scene
                        .materials
                        .get(i.material as usize)
                        .is_some_and(|m| m.kind == 1)
                })
                .filter_map(|i| Puddle::new(scene.meshes.get(i.mesh as usize)?, i.world_from_local))
                .collect(),
            ripple_rng: Rng::new(seed, 20, 0),
            ripple_left: 0.0,
        };
        // Start rain at random lifecycle phases instead of a synchronized first burst.
        for emitter in &mut weather.emitters {
            if emitter.desc.kind != EmitterKind::Rain {
                continue;
            }
            let count = (emitter.desc.rate.max(0.0) * 1.25).min(4000.0) as usize;
            for _ in 0..count.min(4000 - weather.particles.len()) {
                let mut particle = rain(emitter);
                particle.age = emitter.rng.range(0.0, particle.lifetime);
                let pos = Vec3::from_array(particle.position)
                    + Vec3::from_array(particle.velocity) * particle.age;
                particle.position = pos.to_array();
                weather.particles.push(particle);
            }
        }
        weather
    }

    pub fn step(&mut self, dt: f32) {
        for particle in &mut self.particles {
            particle.age += dt;
            let mut position = Vec3::from_array(particle.position);
            let mut velocity = Vec3::from_array(particle.velocity);
            if particle.kind_bits == 1 {
                let fraction = particle.age / particle.lifetime;
                // Jet/buoyant rise -> lateral expansion -> dilute fade.
                let phase = particle.seed_bits as f32 * 0.0001;
                if fraction < 0.3 {
                    velocity.y += dt * 0.9;
                } else {
                    velocity.x += (particle.age * 1.2 + phase).sin() * dt * 0.45;
                    velocity.z += (particle.age * 0.9 + phase).cos() * dt * 0.45;
                    velocity.y *= 1.0 - dt * 0.3;
                }
                particle.size += dt * if fraction < 0.3 { 0.1 } else { 0.45 };
                particle.opacity = if fraction < 0.3 {
                    fraction / 0.3 * 0.45
                } else {
                    0.45 * ((1.0 - fraction) / 0.7).powi(2)
                };
            }
            position += velocity * dt;
            particle.position = position.to_array();
            particle.velocity = velocity.to_array();
        }
        self.particles
            .retain(|p| p.age < p.lifetime && (p.kind_bits == 1 || p.position[1] > 0.0));
        for ripple in &mut self.ripples {
            ripple.age += dt;
        }
        self.ripples.retain(|r| r.age < r.lifetime);
        let mut rain_count = self.particles.iter().filter(|p| p.kind_bits == 0).count();
        let mut steam_count = self.particles.len() - rain_count;
        for emitter in &mut self.emitters {
            if emitter.desc.kind == EmitterKind::Ripple {
                continue;
            }
            if emitter.desc.kind == EmitterKind::Steam {
                emitter.event_left -= dt;
                emitter.burst_left = (emitter.burst_left - dt).max(0.0);
                if emitter.event_left <= 0.0 {
                    emitter.burst_left = emitter.rng.range(0.8, 2.4);
                    emitter.event_left = emitter.rng.range(4.0, 13.0);
                }
                if emitter.burst_left <= 0.0 {
                    continue;
                }
            }
            emitter.credit += emitter.desc.rate.clamp(0.0, 10000.0) * dt;
            let count = emitter.credit.floor() as usize;
            emitter.credit -= count as f32;
            for _ in 0..count {
                match emitter.desc.kind {
                    EmitterKind::Rain if rain_count < 4000 => {
                        let mut particle = rain(emitter);
                        particle.age = emitter.rng.range(0.0, dt.min(particle.lifetime));
                        particle.position = (Vec3::from_array(particle.position)
                            + Vec3::from_array(particle.velocity) * particle.age)
                            .to_array();
                        self.particles.push(particle);
                        rain_count += 1;
                    }
                    EmitterKind::Steam if steam_count < 256 => {
                        self.particles.push(steam(emitter));
                        steam_count += 1;
                    }
                    _ => break,
                }
            }
        }
        // All centres are sampled in the local footprint of actual kind=1 surfaces.
        // Rain/ripple emitter locations never create waves on a dry road.
        self.ripple_left -= dt;
        let enabled = self.emitters.iter().any(|e| {
            matches!(e.desc.kind, EmitterKind::Rain | EmitterKind::Ripple) && e.desc.rate > 0.0
        });
        if enabled
            && !self.puddles.is_empty()
            && self.ripple_left <= 0.0
            && self.ripples.len() < 128
        {
            self.ripple_left = self.ripple_rng.range(0.04, 0.18);
            let index = (self.ripple_rng.unit() * self.puddles.len() as f32) as usize;
            let center = self.puddles[index].sample(&mut self.ripple_rng);
            self.ripples.push(RippleGpu {
                center: center.to_array(),
                age: 0.0,
                lifetime: self.ripple_rng.range(0.6, 1.8),
                amplitude: self.ripple_rng.range(0.002, 0.012),
                wavelength: self.ripple_rng.range(0.08, 0.2),
                speed: self.ripple_rng.range(0.3, 0.8),
            });
        }
    }
}
fn rain(emitter: &mut Emitter) -> ParticleGpu {
    let origin = Vec3::from_array(emitter.desc.position);
    let direction = Vec3::from_array(emitter.desc.direction).normalize_or_zero();
    let velocity = if direction.length_squared() > 0.0 {
        direction * emitter.rng.range(15.0, 24.0)
    } else {
        Vec3::new(-1.0, -19.0, 0.5)
    };
    let position = origin
        + Vec3::new(
            emitter.rng.range(-12.0, 12.0),
            emitter.rng.range(0.0, 8.0),
            emitter.rng.range(-60.0, 60.0),
        );
    let lifetime = if velocity.y < -0.01 {
        (position.y / -velocity.y).clamp(0.05, 4.0)
    } else {
        1.5
    };
    ParticleGpu {
        position: position.to_array(),
        velocity: velocity.to_array(),
        age: 0.0,
        lifetime,
        color: [0.45, 0.6, 0.8],
        opacity: emitter.rng.range(0.1, 0.35),
        size: emitter.rng.range(0.012, 0.025),
        rotation: 0.0,
        kind_bits: 0,
        seed_bits: mix(emitter.rng.0) as u32,
    }
}
fn steam(emitter: &mut Emitter) -> ParticleGpu {
    let jitter = Vec3::new(
        emitter.rng.range(-0.15, 0.15),
        0.0,
        emitter.rng.range(-0.15, 0.15),
    );
    let direction = Vec3::from_array(emitter.desc.direction).normalize_or_zero();
    let velocity = direction * emitter.rng.range(0.5, 1.2) + Vec3::Y * 0.5 + jitter;
    ParticleGpu {
        position: (Vec3::from_array(emitter.desc.position) + jitter).to_array(),
        age: 0.0,
        velocity: velocity.to_array(),
        lifetime: emitter.rng.range(2.4, 5.5),
        color: [0.38, 0.42, 0.5],
        opacity: 0.0,
        size: emitter.rng.range(0.12, 0.3),
        rotation: emitter.rng.range(0.0, std::f32::consts::TAU),
        kind_bits: 1,
        seed_bits: mix(emitter.rng.0) as u32,
    }
}
