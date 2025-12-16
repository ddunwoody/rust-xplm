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

use crate::data::{
    owned::{CreateError, OwnedData},
    Access, ArrayRead, ArrayType, DataRead, DataType, ReadOnly, ReadWrite,
};
use std::marker::PhantomData;

use super::{ValidatedArrayRead, ValidatedData, ValidatedDataRead};

#[derive(Clone, Debug, thiserror::Error)]
pub enum ValidatedCreateError<T, V>
where
    T: DataType + ?Sized,
    V: super::Validator<T::Validation>,
{
    #[error("data validation error: {0:?}")]
    DataInvalid(V::Error),
    #[error("error creating underlying dataref: {0}")]
    CreateError(#[from] CreateError),
}

pub type ValidatedOwnedData<T, V, A = ReadOnly> = ValidatedData<T, V, OwnedData<T, A>>;

macro_rules! impl_validated_owned_data {
    // scalar case
    ($native_type:ty) => {
        impl<V, A> ValidatedOwnedData<$native_type, V, A>
        where
            $native_type: DataType,
            V: super::Validator<$native_type>,
            A: Access,
        {
            /// Creates a new dataref with the provided name containing the default value of T
            pub fn create(name: &str) -> Result<Self, ValidatedCreateError<$native_type, V>>
            where
                $native_type: Default,
            {
                let v = <$native_type>::default();
                V::validate(&v).map_err(|e| ValidatedCreateError::DataInvalid(e))?;
                Ok(Self {
                    dr: OwnedData::create_with_value(name, &v)?,
                    data: PhantomData,
                    validator: PhantomData,
                })
            }
            /// Creates a new dataref with the provided name and value
            pub fn create_with_value(
                name: &str,
                value: &$native_type,
            ) -> Result<Self, ValidatedCreateError<$native_type, V>> {
                V::validate(value).map_err(|e| ValidatedCreateError::DataInvalid(e))?;
                Ok(Self {
                    dr: OwnedData::create_with_value(name, value)?,
                    data: PhantomData,
                    validator: PhantomData,
                })
            }
        }

        impl<V> DataRead<$native_type> for ValidatedOwnedData<$native_type, V, ReadOnly>
        where
            V: super::Validator<$native_type>,
        {
            fn get(&self) -> $native_type {
                self.dr.get()
            }
        }

        impl<V> ValidatedDataRead<$native_type, V>
            for ValidatedOwnedData<$native_type, V, ReadWrite>
        where
            V: super::Validator<$native_type>,
        {
            fn get(&self) -> Result<$native_type, V::Error> {
                let value = self.dr.get();
                V::validate(&value).map(|_| value)
            }
        }
    };
    (array $native_type:ty) => {
        impl<V, A> ValidatedOwnedData<[$native_type], V, A>
        where
            [$native_type]: ArrayType,
            V: super::Validator<$native_type>,
            A: Access,
        {
            /// Creates a new dataref with the provided name containing the default value of T
            pub fn create(
                name: &str,
                len: usize,
            ) -> Result<Self, ValidatedCreateError<$native_type, V>>
            where
                $native_type: Default,
            {
                let value = <$native_type>::default();
                V::validate(&value).map_err(|e| ValidatedCreateError::DataInvalid(e))?;
                let values = vec![value; len];
                Ok(Self {
                    dr: OwnedData::create_with_value(name, &values[..])?,
                    data: PhantomData,
                    validator: PhantomData,
                })
            }
            /// Creates a new dataref with the provided name and value
            pub fn create_with_value(
                name: &str,
                values: &[$native_type],
            ) -> Result<Self, ValidatedCreateError<$native_type, V>> {
                if let Some(e) = values.iter().find_map(|value| V::validate(value).err()) {
                    return Err(ValidatedCreateError::DataInvalid(e));
                }
                Ok(Self {
                    dr: OwnedData::create_with_value(name, values)?,
                    data: PhantomData,
                    validator: PhantomData,
                })
            }
        }

        impl<V> ArrayRead<[$native_type]> for ValidatedOwnedData<[$native_type], V, ReadOnly>
        where
            V: super::Validator<$native_type>,
        {
            fn get_subdata(&self, dest: &mut [$native_type], start_offset: usize) -> usize {
                self.dr.get_subdata(dest, start_offset)
            }
            fn len(&self) -> usize {
                self.dr.len()
            }
        }

        impl<V> ValidatedArrayRead<[$native_type], V>
            for ValidatedOwnedData<[$native_type], V, ReadWrite>
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

impl_validated_owned_data!(bool);
impl_validated_owned_data!(u8);
impl_validated_owned_data!(i8);
impl_validated_owned_data!(u16);
impl_validated_owned_data!(i16);
impl_validated_owned_data!(u32);
impl_validated_owned_data!(i32);
impl_validated_owned_data!(f32);
impl_validated_owned_data!(f64);

impl_validated_owned_data!(array bool);
impl_validated_owned_data!(array u8);
impl_validated_owned_data!(array i8);
impl_validated_owned_data!(array u32);
impl_validated_owned_data!(array i32);
impl_validated_owned_data!(array f32);

#[cfg(test)]
mod tests {
    use crate::data::validated::{owned::ValidatedOwnedData, validator};
    use crate::data::validated::{ValidatedDataRead, ValidatedDataReadWrite};
    use crate::data::ReadWrite;

    #[test]
    fn test_validated_owned_data() {
        type TestDatarefValidator = validator::Range<1, 5>;
        assert!(ValidatedOwnedData::<u32, TestDatarefValidator>::create("test/new/u32").is_err());
        assert!(
            ValidatedOwnedData::<u32, TestDatarefValidator>::create_with_value("test/new/u32", &1)
                .is_ok()
        );
        let mut dr = ValidatedOwnedData::<u32, TestDatarefValidator, ReadWrite>::create_with_value(
            "test/new/u32",
            &1,
        )
        .unwrap();
        assert!(dr.set(2).is_ok());
        assert_eq!(dr.get().unwrap(), 2);
    }
}
