use super::*;
struct Sign {
    material: usize,
    kind: SignKind,
    rng: Rng,
    until_event: f32,
    fault_left: f32,
    phase: f32,
    content_left: f32,
    content: f32,
}
pub(super) struct Signs {
    signs: Vec<Sign>,
    pub states: Vec<MaterialStateGpu>,
}
impl Signs {
    pub fn new(scene: &SceneCpu, seed: u64) -> Self {
        let mut signs = Vec::new();
        for desc in &scene.signs {
            let Some(instance) = scene.instances.iter().find(|i| i.id == desc.instance) else {
                continue;
            };
            let mut rng = Rng::new(seed ^ desc.seed, 3, u64::from(desc.instance));
            signs.push(Sign {
                material: instance.material as usize,
                kind: desc.kind,
                until_event: rng.range(2.0, 9.0),
                fault_left: 0.0,
                phase: rng.range(0.0, 100.0),
                content_left: rng.range(3.0, 7.0),
                content: 0.0,
                rng,
            });
        }
        Self {
            signs,
            states: vec![
                MaterialStateGpu {
                    emission_multiplier: [1.0; 3],
                    ..Default::default()
                };
                scene.materials.len()
            ],
        }
    }
    pub fn step(&mut self, dt: f32) {
        for sign in &mut self.signs {
            sign.phase += dt;
            sign.until_event -= dt;
            sign.fault_left = (sign.fault_left - dt).max(0.0);
            if sign.until_event <= 0.0 {
                sign.fault_left = sign.rng.range(0.04, 0.18);
                sign.until_event = sign.rng.range(2.0, 9.0);
            }
            sign.content_left -= dt;
            if sign.content_left <= 0.0 {
                sign.content = sign.rng.unit();
                sign.content_left = sign.rng.range(3.0, 7.0);
            }
            let Some(state) = self.states.get_mut(sign.material) else {
                continue;
            };
            let before = state.emission_multiplier;
            let fault = if sign.fault_left > 0.0 { 1.0 } else { 0.0 };
            let wave = (sign.phase * 7.7).sin();
            let intensity = match sign.kind {
                SignKind::Tube => 1.0 - fault * 0.9,
                SignKind::Led => 0.78 + sign.content * 0.12,
                SignKind::Hologram => 0.65 + 0.12 * wave - fault * 0.3,
                SignKind::FaultText => 0.9 - fault * 0.5,
            };
            state.emission_multiplier = [intensity; 3];
            state.phase = sign.phase;
            state.fault_amount = if sign.kind == SignKind::FaultText {
                fault.max((wave - 0.85).max(0.0) * 2.0)
            } else {
                fault
            };
            state.scan_offset = (sign.phase * 0.73 + sign.content).fract();
            state.reactive_weight = (before[0] - intensity)
                .abs()
                .max(state.fault_amount)
                .clamp(0.0, 1.0);
        }
    }
    pub fn reactive(&self) -> Vec<u32> {
        self.states
            .iter()
            .enumerate()
            .filter(|(_, s)| s.reactive_weight > 0.05)
            .map(|(i, _)| i as u32)
            .collect()
    }
}
