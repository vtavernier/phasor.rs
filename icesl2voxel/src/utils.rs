use serde_derive::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct BoundingBox<T> {
    pub min_x: T,
    pub min_y: T,
    pub min_z: T,
    pub max_x: T,
    pub max_y: T,
    pub max_z: T,
}

impl<T: num_traits::Num + num_traits::NumCast + std::fmt::Debug + Copy + 'static> BoundingBox<T> {
    pub fn size(&self) -> nalgebra::Vector3<T> {
        nalgebra::Vector3::new(
            self.max_x - self.min_x,
            self.max_y - self.min_y,
            self.max_z - self.min_z,
        )
    }

    pub fn center(&self) -> nalgebra::Vector3<T> {
        nalgebra::Vector3::new(
            (self.max_x + self.min_x) / num_traits::NumCast::from(2.0).unwrap(),
            (self.max_y + self.min_y) / num_traits::NumCast::from(2.0).unwrap(),
            (self.max_z + self.min_z) / num_traits::NumCast::from(2.0).unwrap(),
        )
    }

    pub fn min(&self) -> nalgebra::Vector3<T> {
        nalgebra::Vector3::new(self.min_x, self.min_y, self.min_z)
    }

    pub fn max(&self) -> nalgebra::Vector3<T> {
        nalgebra::Vector3::new(self.max_x, self.max_y, self.max_z)
    }
}

impl<'a, T, U> From<T> for BoundingBox<U>
where
    T: IntoIterator<Item = (&'a nalgebra::Vector3<U>, &'a nalgebra::Vector3<U>)>,
    U: num_traits::bounds::Bounded
        + num_traits::Num
        + num_traits::Float
        + std::fmt::Debug
        + Copy
        + 'static,
{
    fn from(it: T) -> Self {
        let mut min_x = <U as num_traits::bounds::Bounded>::max_value();
        let mut min_y = <U as num_traits::bounds::Bounded>::max_value();
        let mut min_z = <U as num_traits::bounds::Bounded>::max_value();
        let mut max_x = <U as num_traits::bounds::Bounded>::min_value();
        let mut max_y = <U as num_traits::bounds::Bounded>::min_value();
        let mut max_z = <U as num_traits::bounds::Bounded>::min_value();

        for (pt1, pt2) in it {
            for pt in [pt1, pt2].iter() {
                min_x = min_x.min(pt[0]);
                min_y = min_y.min(pt[1]);
                min_z = min_z.min(pt[2]);
                max_x = max_x.max(pt[0]);
                max_y = max_y.max(pt[1]);
                max_z = max_z.max(pt[2]);
            }
        }

        Self {
            min_x,
            min_y,
            min_z,
            max_x,
            max_y,
            max_z,
        }
    }
}
