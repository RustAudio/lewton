pub trait Samples {
	fn num_samples(&self) -> usize;
	fn truncate(&mut self, limit :usize);
	fn from_floats(floats :Vec<Vec<f32>>) -> Self;
}

impl<S :Sample> Samples for Vec<Vec<S>> {
	fn num_samples(&self) -> usize {
		self[0].len()
	}
	fn truncate(&mut self, limit :usize) {
		for ch in self.iter_mut() {
			if limit < ch.len() {
				ch.truncate(limit);
			}
		}
	}

	fn from_floats(floats :Vec<Vec<f32>>) -> Self {
		floats.into_iter()
			.map(|samples| {
				samples.into_iter()
					.map(S::from_float)
					.collect()
			}).collect()
	}
}

pub trait Sample {
	fn from_float(fl :f32) -> Self;
}

impl Sample for f32 {
	fn from_float(fl :f32) -> Self {
		fl
	}
}

impl Sample for i16 {
	fn from_float(fl :f32) -> Self {
		let fl = fl * 32768.0;
		if fl > 32767. {
			32767
		} else if fl < -32768. {
			-32768
		} else {
			fl as i16
		}
	}
}
