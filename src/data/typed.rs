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

use super::{ArrayRead, ArrayReadWrite, ArrayType, DataRead, DataReadWrite};

pub mod borrowed;
pub mod owned;
#[cfg(feature = "uom_conv")]
pub mod uom_conv;

/// Trait which must be implemented by the conversion type to be able to read from typed datarefs
/// and convert them into the associated data type.
pub trait InputUnitConversion<X, R> {
    type Error;
    fn try_conv_in(value: X) -> Result<R, Self::Error>;
}

/// Trait which must be implemented by the conversion type to be able to write to typed datarefs
/// and convert the associated data type into the payload in the dataref.
pub trait OutputUnitConversion<R, X> {
    fn conv_out(value: &R) -> X;
}

/// This encapsulates a dataref (either owned or borrowed), which provides type-safe access
/// mechanisms for ingesting and outputting Rust data types. This way you can write a
/// non-primitive data type into a dataref, and have the conversion + validation happen
/// automatically.
///
/// For convenience, it is easiest to use the canned types for either borrowed (`TypedDataRef`)
/// or owned (`TypedOwnedData`) datarefs. Please see those types for examples on how to use
/// them.
#[derive(Copy, Clone, Debug)]
pub struct TypedData<X: ?Sized, R, Conversion, Dref> {
    dr: Dref,
    data: PhantomData<X>,
    rust_type: PhantomData<R>,
    conv: PhantomData<Conversion>,
}

pub trait TypedDataRead<X, R, C>
where
    C: InputUnitConversion<X, R>,
{
    fn get(&self) -> Result<R, C::Error>;
}

pub trait TypedDataReadWrite<X, R, C>
where
    C: OutputUnitConversion<R, X>,
{
    fn set(&mut self, value: R);
}

pub trait TypedArrayRead<X, R, C>
where
    X: ArrayType + ?Sized,
    C: InputUnitConversion<X::Validation, R>,
{
    fn get(&self) -> Result<Vec<R>, C::Error> {
        self.get_subdata(..)
    }
    fn get_subdata(&self, range: impl std::ops::RangeBounds<usize>) -> Result<Vec<R>, C::Error>;
}

pub trait TypedArrayReadWrite<X, R, C>
where
    X: ArrayType + ?Sized,
    C: OutputUnitConversion<R, X::Validation>,
{
    fn set(&mut self, values: &[R]) {
        self.set_subdata(values, 0);
    }
    fn set_subdata(&mut self, values: &[R], offset: usize);
}

macro_rules! impl_typed_data {
    ($native_type:ty) => {
        impl<R, C, Dref> TypedDataRead<$native_type, R, C> for TypedData<$native_type, R, C, Dref>
        where
            C: InputUnitConversion<$native_type, R>,
            Dref: DataRead<$native_type>,
        {
            fn get(&self) -> Result<R, C::Error> {
                C::try_conv_in(self.dr.get())
            }
        }
        impl<R, C, Dref> TypedDataReadWrite<$native_type, R, C>
            for TypedData<$native_type, R, C, Dref>
        where
            C: OutputUnitConversion<R, $native_type>,
            Dref: DataReadWrite<$native_type>,
        {
            fn set(&mut self, value: R) {
                self.dr.set(C::conv_out(&value));
            }
        }
    };
    (array $native_type:ty) => {
        impl<R, C, Dref> TypedArrayReadWrite<[$native_type], R, C>
            for TypedData<[$native_type], R, C, Dref>
        where
            C: OutputUnitConversion<R, $native_type>,
            Dref: ArrayReadWrite<[$native_type]>,
        {
            fn set_subdata(&mut self, values: &[R], start_offset: usize) {
                let values = values
                    .iter()
                    .map(|value| C::conv_out(value))
                    .collect::<Vec<_>>();
                self.dr.set_subdata(&values, start_offset);
            }
        }
        impl<R, C, Dref> TypedArrayRead<[$native_type], R, C>
            for TypedData<[$native_type], R, C, Dref>
        where
            C: InputUnitConversion<$native_type, R>,
            Dref: ArrayRead<[$native_type]>,
        {
            fn get_subdata(
                &self,
                range: impl std::ops::RangeBounds<usize>,
            ) -> Result<Vec<R>, C::Error> {
                self.dr
                    .as_vec_subdata(range)
                    .into_iter()
                    .map(|value| C::try_conv_in(value))
                    .collect()
            }
        }
    };
}

impl_typed_data!(bool);
impl_typed_data!(i8);
impl_typed_data!(u8);
impl_typed_data!(i16);
impl_typed_data!(u16);
impl_typed_data!(i32);
impl_typed_data!(u32);
impl_typed_data!(f32);
impl_typed_data!(f64);

impl_typed_data!(array bool);
impl_typed_data!(array i8);
impl_typed_data!(array u8);
impl_typed_data!(array i32);
impl_typed_data!(array u32);
impl_typed_data!(array f32);
