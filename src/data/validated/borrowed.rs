// Licensed under either of:
/*
 * Apache License, Version 2.0:
 *
 * Copyright 2025 Sašo Kiselkov
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *      http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */
/*
 * MIT license:
 *
 * Copyright (c) 2025 Sašo Kiselkov
 *
 * Permission is hereby granted, free of charge, to any pony obtaining a
 * copy of this software and associated documentation files (the "Software"),
 * to deal in the Software without restriction, including without limitation
 * the rights to use, copy, modify, merge, publish, distribute, sublicense,
 * and/or sell copies of the Software, and to permit ponies to whom the
 * Software is furnished to do so, subject to the following conditions:
 *
 * The above copyright notice and this permission notice shall be included in
 * all copies or substantial portions of the Software.
 *
 * THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
 * IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
 * FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL
 * THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
 * LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
 * FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
 * DEALINGS IN THE SOFTWARE.
 */

use std::marker::PhantomData;

use crate::data::{
    borrowed::{DataRef, FindError},
    ArrayRead, DataRead, DataType, ReadOnly, ReadWrite,
};

use super::{ValidatedArrayRead, ValidatedData, ValidatedDataRead};

/// A dataref created by X-Plane or another plugin, with support for flexible input/output
/// validators.
///
/// # Example
/// ```no_run
/// use xplm::data::ReadWrite;
/// use xplm::data::validated::{validator, ValidatedDataRead, ValidatedDataReadWrite};
/// use xplm::data::validated::borrowed::ValidatedDataRef;
///
/// // Create a numerical range checker using the provided `RangeInclusive` validator
/// // (requires the `number_validation` feature be enable).
/// // NOTE: you can combine multiple validators together using the `validator::And` and
/// // `validator::Or` meta-validators.
/// type CheckRangeA = validator::RangeInclusive<0, 10>;
///
/// // Look up the dataref and associate it with the range checker.
/// let mut dr: ValidatedDataRef<i32, CheckRangeA, ReadWrite> =
///     ValidatedDataRef::find("test/i32")
///         .unwrap()
///         .writeable()
///         .unwrap();
///
/// // We can now attempt to read the dataref. The returned data will first be
/// // validated by the range checker.
/// match dr.get() {
///     Ok(value) => println!("Read dataref value {value}"),
///     Err(e) => println!("Reading dataref failed due to input validation error: {e:?}"),
/// }
///
/// // We can try to set something into the dataref. The validator will check that our
/// // input conforms to the criteria.
/// assert!(dr.set(7).is_ok());
/// // If we try to put something invalid into the dataref, we'll get an error.
/// assert!(dr.set(1000).is_err());
/// ```
pub type ValidatedDataRef<T, V, A = ReadOnly> = ValidatedData<T, V, DataRef<T, A>>;

impl<T, V> ValidatedDataRef<T, V, ReadOnly>
where
    T: DataType + ?Sized,
    V: super::Validator<T::Validation>,
{
    /// Performs a lookup for this dataref and returns it if found, or an error otherwise.
    pub fn find(name: &str) -> Result<Self, FindError> {
        Ok(Self {
            dr: DataRef::find(name)?,
            data: PhantomData,
            validator: PhantomData,
        })
    }
    /// Makes this dataref writable
    ///
    /// Returns an error if the dataref cannot be written.
    pub fn writeable(self) -> Result<ValidatedDataRef<T, V, ReadWrite>, FindError> {
        Ok(ValidatedData {
            dr: self.dr.writeable()?,
            data: PhantomData,
            validator: PhantomData,
        })
    }
}

macro_rules! impl_validated_dataref {
    ($native_type:ty) => {
        impl<V, A> ValidatedDataRead<$native_type, V> for ValidatedDataRef<$native_type, V, A>
        where
            V: super::Validator<$native_type>,
        {
            fn get(&self) -> Result<$native_type, V::Error> {
                let value = self.dr.get();
                V::validate(&value)?;
                Ok(value)
            }
        }
    };
    (array $native_type:ty) => {
        impl<V> ValidatedArrayRead<[$native_type], V> for ValidatedDataRef<[$native_type], V>
        where
            V: super::Validator<$native_type>,
        {
            fn get_subdata(
                &self,
                range: impl std::ops::RangeBounds<usize>,
            ) -> Result<Vec<$native_type>, V::Error> {
                let tmp = self.dr.as_vec_subdata(range);
                if let Some(e) = tmp.iter().find_map(|value| V::validate(value).err()) {
                    return Err(e);
                }
                Ok(tmp)
            }
            fn len(&self) -> usize {
                self.dr.len()
            }
        }
    };
}

impl_validated_dataref!(bool);
impl_validated_dataref!(i8);
impl_validated_dataref!(u8);
impl_validated_dataref!(i16);
impl_validated_dataref!(u16);
impl_validated_dataref!(i32);
impl_validated_dataref!(u32);
impl_validated_dataref!(f32);
impl_validated_dataref!(f64);

impl_validated_dataref!(array bool);
impl_validated_dataref!(array u8);
impl_validated_dataref!(array i8);
impl_validated_dataref!(array i32);
impl_validated_dataref!(array u32);
impl_validated_dataref!(array f32);

#[cfg(test)]
mod tests {
    use super::{DataRef, ValidatedDataRef};
    use crate::data::validated::{validator, ValidatedDataRead, ValidatedDataReadWrite};
    use crate::data::{DataReadWrite, ReadWrite};

    #[test]
    fn test_validated_dataref() {
        let _dr_lock = crate::test_stubs::DATAREF_SYS_LOCK.lock();

        let mut dr: ValidatedDataRef<i32, validator::Range<0, 5>, ReadWrite> =
            ValidatedDataRef::find("test/i32")
                .unwrap()
                .writeable()
                .unwrap();
        assert!(dr.set(4).is_ok());
        assert!(dr.set(5).is_err());

        let mut unvalidated_dr: DataRef<f64, ReadWrite> =
            DataRef::find("test/f64").unwrap().writeable().unwrap();
        unvalidated_dr.set(f64::NAN);

        let mut dr: ValidatedDataRef<f64, validator::NormalFloat, ReadWrite> =
            ValidatedDataRef::find("test/f64")
                .unwrap()
                .writeable()
                .unwrap();
        assert!(dr.get().is_err());
        assert!(dr.set(4.0).is_ok());
        assert!(dr.set(f64::NAN).is_err());
    }
}
