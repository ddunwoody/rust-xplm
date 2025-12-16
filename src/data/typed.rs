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

use super::{ArrayRead, ArrayReadWrite, ArrayType, DataRead, DataReadWrite, DataType};

pub mod borrowed;
pub mod owned;

pub trait UnitConversion<T> {
    fn conv(value: T) -> T;
}

pub struct PassthruConv {}
impl<T> UnitConversion<T> for PassthruConv {
    fn conv(value: T) -> T {
        value
    }
}

pub struct TypedData<X, R, Dref, Cin = PassthruConv, Cout = PassthruConv>
where
    X: DataType + ?Sized,
{
    dr: Dref,
    data: PhantomData<X>,
    rust_type: PhantomData<R>,
    conv_in: PhantomData<Cin>,
    conv_out: PhantomData<Cout>,
}

pub trait TypedDataRead<X, R>
where
    X: DataType,
    R: TryFrom<X::Storage>,
{
    fn get(&self) -> Result<R, R::Error>;
}

pub trait TypedDataReadWrite<X, R>
where
    X: DataType,
    R: Into<X::Storage>,
{
    fn set(&mut self, value: R);
}

pub trait TypedArrayRead<X, R>
where
    X: ArrayType + ?Sized,
    R: TryFrom<X::Element>,
{
    fn get(&self) -> Result<Vec<R>, R::Error> {
        self.get_subdata(..)
    }
    fn get_subdata(&self, range: impl std::ops::RangeBounds<usize>) -> Result<Vec<R>, R::Error>;
}

pub trait TypedArrayReadWrite<X, R>
where
    X: ArrayType + ?Sized,
    R: Into<X::Element>,
{
    fn set(&mut self, values: impl Iterator<Item = R>) {
        self.set_subdata(values, 0);
    }
    fn set_subdata(&mut self, values: impl Iterator<Item = R>, offset: usize);
}

macro_rules! impl_typed_data {
    ($native_type:ty) => {
        impl<R, Dref, Cin, Cout> TypedDataRead<$native_type, R>
            for TypedData<$native_type, R, Dref, Cin, Cout>
        where
            R: TryFrom<$native_type>,
            Dref: DataRead<$native_type>,
            Cin: UnitConversion<$native_type>,
        {
            fn get(&self) -> Result<R, R::Error> {
                R::try_from(Cin::conv(self.dr.get()))
            }
        }
        impl<R, Dref, Cin, Cout> TypedDataReadWrite<$native_type, R>
            for TypedData<$native_type, R, Dref, Cin, Cout>
        where
            R: Into<$native_type>,
            Dref: DataReadWrite<$native_type>,
            Cout: UnitConversion<$native_type>,
        {
            fn set(&mut self, value: R) {
                self.dr.set(Cout::conv(value.into()));
            }
        }
    };
    (array $native_type:ty) => {
        impl<R, Dref, Cin, Cout> TypedArrayRead<[$native_type], R>
            for TypedData<[$native_type], R, Dref, Cin, Cout>
        where
            [$native_type]: ArrayType,
            R: TryFrom<$native_type> + Into<$native_type>,
            Dref: ArrayRead<[$native_type]>,
            Cin: UnitConversion<$native_type>,
        {
            fn get_subdata(
                &self,
                range: impl std::ops::RangeBounds<usize>,
            ) -> Result<Vec<R>, R::Error> {
                self.dr
                    .as_vec_subdata(range)
                    .into_iter()
                    .map(|item| R::try_from(Cin::conv(item)))
                    .collect()
            }
        }

        impl<R, Dref, Cin, Cout> TypedArrayReadWrite<[$native_type], R>
            for TypedData<[$native_type], R, Dref, Cin, Cout>
        where
            R: Into<$native_type>,
            Dref: ArrayReadWrite<[$native_type]>,
            Cout: UnitConversion<$native_type>,
        {
            fn set_subdata(&mut self, values: impl Iterator<Item = R>, start_offset: usize) {
                let values = values
                    .map(|value| Cout::conv(value.into()))
                    .collect::<Vec<_>>();
                self.dr.set_subdata(&values, start_offset);
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
impl_typed_data!(array u8);
impl_typed_data!(array i32);
impl_typed_data!(array u32);
impl_typed_data!(array f32);

#[cfg(test)]
mod tests {
    #[test]
    fn test_typed_dataref() {
        use super::{TypedArrayRead, TypedArrayReadWrite, TypedDataRead, TypedDataReadWrite};
        use crate::data::borrowed::TypedDataRef;
        #[derive(derive_more::TryFrom, Copy, Clone, Debug, PartialEq, Eq)]
        #[try_from(repr)]
        #[repr(i32)]
        enum ValidValues {
            A,
            B,
            C,
        }
        impl From<ValidValues> for i32 {
            fn from(value: ValidValues) -> Self {
                value as _
            }
        }
        impl From<ValidValues> for u8 {
            fn from(value: ValidValues) -> Self {
                value as _
            }
        }
        let mut dr = TypedDataRef::<i32, ValidValues>::find("test/i32")
            .unwrap()
            .writeable()
            .unwrap();
        let en = ValidValues::C;
        dr.set(en);
        assert_ne!(dr.get().unwrap(), ValidValues::A);
        assert_ne!(dr.get().unwrap(), ValidValues::B);
        assert_eq!(dr.get().unwrap(), ValidValues::C);

        let mut array_dr = TypedDataRef::<[i32], ValidValues>::find("test/i32array")
            .unwrap()
            .writeable()
            .unwrap();
        let en = ValidValues::C;
        array_dr.set(std::iter::once(en));
        let en_out = array_dr.get_subdata(0..1).unwrap();
        assert_eq!(en_out, vec![ValidValues::C]);
    }
}
