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

use crate::data::owned::CreateError;
use crate::data::{owned::OwnedData, ReadOnly};
use crate::data::{Access, ArrayType, DataType};

use std::marker::PhantomData;

use super::{PassthruConv, TypedData};

pub type TypedOwnedData<X, R, A = ReadOnly, Cin = PassthruConv, Cout = PassthruConv> =
    TypedData<X, R, OwnedData<X, A>, Cin, Cout>;

macro_rules! impl_typed_owned_data {
    ($native_type:ty) => {
        impl<R, A, Cin, Cout> TypedOwnedData<$native_type, R, A, Cin, Cout>
        where
            $native_type: DataType,
            A: Access,
        {
            /// Creates a new dataref with the provided name containing the default value of T
            pub fn create(name: &str) -> Result<Self, CreateError>
            where
                R: Default + Into<$native_type>,
                Cout: super::UnitConversion<$native_type>,
            {
                let v = Cout::conv(R::default().into());
                Ok(Self {
                    dr: OwnedData::create_with_value(name, &v)?,
                    data: PhantomData,
                    rust_type: PhantomData,
                    conv_in: PhantomData,
                    conv_out: PhantomData,
                })
            }
            /// Creates a new dataref with the provided name and value
            pub fn create_with_value(name: &str, value: R) -> Result<Self, CreateError>
            where
                R: Into<$native_type>,
                Cout: super::UnitConversion<$native_type>,
            {
                Ok(Self {
                    dr: OwnedData::create_with_value(name, &Cout::conv(value.into()))?,
                    data: PhantomData,
                    rust_type: PhantomData,
                    conv_in: PhantomData,
                    conv_out: PhantomData,
                })
            }
        }
    };
    (array $native_type:ty) => {
        impl<R, A, Cin, Cout> TypedOwnedData<[$native_type], R, A, Cin, Cout>
        where
            [$native_type]: ArrayType,
            A: Access,
        {
            pub fn create(name: &str, len: usize) -> Result<Self, CreateError>
            where
                R: Default + Into<$native_type>,
                Cout: super::UnitConversion<$native_type>,
            {
                let v = Cout::conv(R::default().into());
                let values = vec![v; len];
                Ok(Self {
                    dr: OwnedData::create_with_value(name, &values[..])?,
                    data: PhantomData,
                    rust_type: PhantomData,
                    conv_in: PhantomData,
                    conv_out: PhantomData,
                })
            }
            /// Creates a new dataref with the provided name and value
            pub fn create_with_value(
                name: &str,
                values: impl Iterator<Item = R>,
            ) -> Result<Self, CreateError>
            where
                R: Into<$native_type>,
                Cout: super::UnitConversion<$native_type>,
            {
                let values = values
                    .map(|value| Cout::conv(value.into()))
                    .collect::<Vec<_>>();
                Ok(Self {
                    dr: OwnedData::create_with_value(name, &values[..])?,
                    data: PhantomData,
                    rust_type: PhantomData,
                    conv_in: PhantomData,
                    conv_out: PhantomData,
                })
            }
        }
    };
}

impl_typed_owned_data!(bool);
impl_typed_owned_data!(i8);
impl_typed_owned_data!(u8);
impl_typed_owned_data!(i16);
impl_typed_owned_data!(u16);
impl_typed_owned_data!(i32);
impl_typed_owned_data!(u32);
impl_typed_owned_data!(f32);
impl_typed_owned_data!(f64);

impl_typed_owned_data!(array bool);
impl_typed_owned_data!(array u8);
impl_typed_owned_data!(array u32);
impl_typed_owned_data!(array i32);
impl_typed_owned_data!(array f32);

#[cfg(test)]
mod tests {
    use crate::data::typed::{owned::TypedOwnedData, TypedDataRead};

    #[test]
    fn test_typed_owned_data() {
        #[derive(derive_more::TryFrom, Default)]
        #[try_from(repr)]
        #[repr(u32)]
        enum ValidValues {
            #[default]
            A,
            B,
            C,
        }
        impl From<ValidValues> for u32 {
            fn from(value: ValidValues) -> Self {
                value as _
            }
        }
        let dr = TypedOwnedData::<u32, ValidValues>::create("test/new/u32").unwrap();

        match dr.get() {
            Ok(ValidValues::A) => (),
            Ok(ValidValues::B) => (),
            Ok(ValidValues::C) => (),
            Err(_) => todo!(),
        }
    }
}
