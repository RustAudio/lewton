pub trait Samples {
	fn num_samples(&self) -> usize;
	fn truncate(&mut self, limit :usize);
	fn from_floats(floats :Vec<Vec<f32>>) -> Self;
}

fn non_interleaved_truncation<T>(v :&mut Vec<Vec<T>>, limit :usize) {
	for ch in v.iter_mut() {
		if limit < ch.len() {
			ch.truncate(limit);
		}
	}
}

impl Samples for Vec<Vec<i16>> {
	fn num_samples(&self) -> usize {
		self[0].len()
	}
	fn truncate(&mut self, limit :usize) {
		non_interleaved_truncation(self, limit);
	}

	fn from_floats(floats :Vec<Vec<f32>>) -> Self {
		floats.into_iter()
			.map(|samples| {
				samples.iter()
					.map(|s| {
						let s = s * 32768.0;
						if s > 32767. {
							32767
						} else if s < -32768. {
							-32768
						} else {
							s as i16
						}
					})
					.collect()
			}).collect()
	}
}

impl Samples for Vec<Vec<f32>> {
	fn num_samples(&self) -> usize {
		self[0].len()
	}
	fn truncate(&mut self, limit :usize) {
		non_interleaved_truncation(self, limit);
	}
	fn from_floats(floats :Vec<Vec<f32>>) -> Self {
		floats
	}
}
