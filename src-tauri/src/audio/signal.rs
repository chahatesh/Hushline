// SPDX-License-Identifier: MIT
//! Preserve voice when a device exposes silent or opposite-polarity channels.
//! No gating, denoising, or removal of quiet samples.
#[derive(Default)]
pub(super) struct MonoMixer {
    selected: Option<usize>,
}
impl MonoMixer {
    pub fn reset(&mut self) {
        self.selected = None;
    }
    pub fn append(
        &mut self,
        data: &[u8],
        channels: usize,
        bytes: usize,
        float: bool,
        mut append: impl FnMut(f32),
    ) {
        let mut power = [0.0f64; 32];
        let mut mixed_power = 0.0;
        for frame in data.chunks_exact(channels * bytes) {
            let mut sum = 0.0;
            for (channel, value) in frame.chunks_exact(bytes).enumerate() {
                let value = decode(value, float) as f64;
                power[channel] += value * value;
                sum += value;
            }
            mixed_power += (sum / channels as f64).powi(2);
        }
        let dominant = (0..channels)
            .max_by(|a, b| power[*a].total_cmp(&power[*b]))
            .unwrap_or(0);
        let loudest = power[dominant];
        // Exclude only channels effectively at digital silence. This prevents
        // unused input-interface channels from attenuating the microphone.
        let active = power[..channels]
            .iter()
            .filter(|p| **p > loudest * 1e-6 && **p > 1e-15)
            .count()
            .max(1);
        let normalized_power = mixed_power * (channels as f64 / active as f64).powi(2);
        if self.selected.is_none() && loudest > 1e-12 && normalized_power < loudest * 0.1 {
            self.selected = Some(dominant);
        }
        if self
            .selected
            .is_some_and(|channel| power[channel] < loudest * 0.001)
        {
            self.selected = Some(dominant);
        }
        for frame in data.chunks_exact(channels * bytes) {
            let mono = if let Some(channel) = self.selected {
                decode(&frame[channel * bytes..(channel + 1) * bytes], float)
            } else {
                frame
                    .chunks_exact(bytes)
                    .map(|v| decode(v, float))
                    .sum::<f32>()
                    / active as f32
            };
            append(mono.clamp(-1.0, 1.0));
        }
    }
}
pub(super) fn decode(bytes: &[u8], float: bool) -> f32 {
    let value = if float {
        f32::from_le_bytes(bytes.try_into().unwrap())
    } else {
        match bytes.len() {
            2 => i16::from_le_bytes(bytes.try_into().unwrap()) as f32 / 32768.0,
            3 => {
                ((bytes[0] as i32 | (bytes[1] as i32) << 8 | (bytes[2] as i32) << 16) << 8 >> 8)
                    as f32
                    / 8_388_608.0
            }
            4 => i32::from_le_bytes(bytes.try_into().unwrap()) as f32 / 2_147_483_648.0,
            _ => 0.0,
        }
    };
    if value.is_finite() {
        value.clamp(-1.0, 1.0)
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn packet(channels: &[Vec<f32>]) -> Vec<u8> {
        (0..channels[0].len())
            .flat_map(|i| channels.iter().flat_map(move |c| c[i].to_le_bytes()))
            .collect()
    }
    #[test]
    fn opposite_polarity_channels_do_not_erase_voice() {
        let voice: Vec<f32> = (0..480).map(|n| (n as f32 * 0.2).sin() * 0.4).collect();
        let inverted = voice.iter().map(|v| -*v).collect();
        let mut result = vec![];
        MonoMixer::default().append(&packet(&[voice.clone(), inverted]), 2, 4, true, |v| {
            result.push(v)
        });
        assert_eq!(result.len(), voice.len());
        let power: f32 = result.iter().map(|v| v * v).sum();
        let expected: f32 = voice.iter().map(|v| v * v).sum();
        assert!((power - expected).abs() < 0.0001);
    }
    #[test]
    fn silent_and_invalid_channels_do_not_attenuate_a_working_microphone() {
        for unused in [0.0, f32::NAN, f32::INFINITY] {
            let mut result = vec![];
            MonoMixer::default().append(
                &packet(&[vec![unused; 480], vec![0.1; 480]]),
                2,
                4,
                true,
                |v| result.push(v),
            );
            assert!(result.iter().all(|v| (*v - 0.1).abs() < 1e-6));
        }
    }
    #[test]
    fn coherent_channels_and_quiet_words_stay_intact() {
        let voice = vec![0.2, 0.02, 0.002, 0.0002, 0.0, -0.2];
        let mut result = vec![];
        MonoMixer::default().append(&packet(&[voice.clone(), voice.clone()]), 2, 4, true, |v| {
            result.push(v)
        });
        assert_eq!(result, voice);
    }
    #[test]
    fn channel_choice_is_stable_until_the_selected_channel_goes_silent() {
        let mut mixer = MonoMixer::default();
        mixer.append(
            &packet(&[vec![0.3; 100], vec![-0.3; 100]]),
            2,
            4,
            true,
            |_| {},
        );
        let selected = mixer.selected.unwrap();
        mixer.append(
            &packet(&[vec![0.31; 100], vec![-0.29; 100]]),
            2,
            4,
            true,
            |_| {},
        );
        assert_eq!(mixer.selected, Some(selected));
        mixer.reset();
        assert!(mixer.selected.is_none());
    }
}
