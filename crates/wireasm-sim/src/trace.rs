//! Time-domain traces and AC sweeps produced by the simulator.
//!
//! These types are the data behind the oscilloscope, the logic analyzer and
//! the Bode plot. They are plain vectors so the UI can draw them directly and
//! tests can compare numbers.

use std::collections::BTreeMap;

use crate::linalg::C;

#[derive(Clone, Debug, Default)]
pub struct Trace {
    pub time: Vec<f64>,
    pub series: BTreeMap<String, Vec<f64>>,
}

impl Trace {
    pub fn is_empty(&self) -> bool {
        self.time.is_empty()
    }

    pub fn len(&self) -> usize {
        self.time.len()
    }

    pub fn names(&self) -> Vec<&str> {
        self.series.keys().map(|s| s.as_str()).collect()
    }

    pub fn peak(&self, name: &str) -> Option<f64> {
        self.series
            .get(name)?
            .iter()
            .fold(None, |acc, v| Some(acc.map_or(*v, |a: f64| a.max(*v))))
    }

    pub fn min(&self, name: &str) -> Option<f64> {
        self.series
            .get(name)?
            .iter()
            .fold(None, |acc, v| Some(acc.map_or(*v, |a: f64| a.min(*v))))
    }

    pub fn mean(&self, name: &str) -> Option<f64> {
        let s = self.series.get(name)?;
        if s.is_empty() {
            return None;
        }
        Some(s.iter().sum::<f64>() / s.len() as f64)
    }

    /// Root mean square, using the sample times as weights when possible.
    pub fn rms(&self, name: &str) -> Option<f64> {
        let s = self.series.get(name)?;
        if s.is_empty() {
            return None;
        }
        if self.time.len() == s.len() && self.time.len() > 1 {
            let mut sum = 0.0;
            let mut weight = 0.0;
            let mut prev_t = self.time[0];
            for (i, v) in s.iter().enumerate() {
                let dt = if i == 0 {
                    self.time[1] - self.time[0]
                } else {
                    self.time[i] - prev_t
                };
                sum += v * v * dt;
                weight += dt;
                prev_t = self.time[i];
            }
            if weight > 0.0 {
                return Some((sum / weight).sqrt());
            }
        }
        Some((s.iter().map(|v| v * v).sum::<f64>() / s.len() as f64).sqrt())
    }

    /// Time of the first upward crossing of `level`, linearly interpolated.
    pub fn rising_crossing(&self, name: &str, level: f64) -> Option<f64> {
        let s = self.series.get(name)?;
        for i in 1..s.len().min(self.time.len()) {
            if s[i - 1] < level && s[i] >= level {
                let dv = s[i] - s[i - 1];
                if dv.abs() < 1e-15 {
                    return Some(self.time[i]);
                }
                let f = (level - s[i - 1]) / dv;
                return Some(self.time[i - 1] + f * (self.time[i] - self.time[i - 1]));
            }
        }
        None
    }

    pub fn to_csv(&self) -> String {
        let names: Vec<&str> = self.series.keys().map(|s| s.as_str()).collect();
        let mut out = String::from("time");
        for n in &names {
            out.push(',');
            out.push_str(n);
        }
        out.push('\n');
        for i in 0..self.time.len() {
            out.push_str(&format!("{:.9}", self.time[i]));
            for n in &names {
                out.push(',');
                if let Some(v) = self.series.get(*n).and_then(|s| s.get(i)) {
                    out.push_str(&format!("{v:.9}"));
                }
            }
            out.push('\n');
        }
        out
    }
}

#[derive(Clone, Debug, Default)]
pub struct AcSweep {
    pub frequency: Vec<f64>,
    /// Complex phasor at each frequency, one vector per probe.
    pub series: BTreeMap<String, Vec<C>>,
}

impl AcSweep {
    pub fn magnitude(&self, name: &str) -> Vec<f64> {
        self.series
            .get(name)
            .map(|s| s.iter().map(|c| c.abs()).collect())
            .unwrap_or_default()
    }

    pub fn phase_deg(&self, name: &str) -> Vec<f64> {
        self.series
            .get(name)
            .map(|s| s.iter().map(|c| c.arg().to_degrees()).collect())
            .unwrap_or_default()
    }

    /// Magnitude in dB relative to `reference` (usually the source amplitude).
    pub fn magnitude_db(&self, name: &str, reference: f64) -> Vec<f64> {
        self.magnitude(name)
            .into_iter()
            .map(|m| 20.0 * (m / reference).log10())
            .collect()
    }

    pub fn to_csv(&self) -> String {
        let names: Vec<&str> = self.series.keys().map(|s| s.as_str()).collect();
        let mut out = String::from("frequency");
        for n in &names {
            out.push_str(&format!(",{n}_mag,{n}_phase"));
        }
        out.push('\n');
        for (i, f) in self.frequency.iter().enumerate() {
            out.push_str(&format!("{f:.9}"));
            for n in &names {
                if let Some(c) = self.series.get(*n).and_then(|s| s.get(i)) {
                    out.push_str(&format!(",{:.9},{:.9}", c.abs(), c.arg().to_degrees()));
                } else {
                    out.push_str(",,");
                }
            }
            out.push('\n');
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn trace() -> Trace {
        let mut t = Trace {
            time: vec![0.0, 1.0, 2.0, 3.0],
            ..Trace::default()
        };
        t.series.insert("v".into(), vec![0.0, 1.0, 2.0, 1.0]);
        t
    }

    #[test]
    fn measurements() {
        let t = trace();
        assert_eq!(t.peak("v"), Some(2.0));
        assert_eq!(t.min("v"), Some(0.0));
        assert_eq!(t.mean("v"), Some(1.0));
        assert!((t.rising_crossing("v", 0.5).unwrap() - 0.5).abs() < 1e-12);
    }
}
