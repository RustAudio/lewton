pub trait Samples {
	fn from_floats(floats :Vec<Vec<f32>>) -> Self;
}

impl Samples for Vec<Vec<i16>> {
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
	fn from_floats(floats :Vec<Vec<f32>>) -> Self {
		floats
	}
}
