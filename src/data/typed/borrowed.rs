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
    DataType, ReadOnly, ReadWrite,
};

pub type TypedDataRef<X, R, C, A = ReadOnly> = super::TypedData<X, R, C, DataRef<X, A>>;

impl<X, R, C> TypedDataRef<X, R, C, ReadOnly>
where
    X: DataType + ?Sized,
{
    pub fn find<S: AsRef<str>>(name: S) -> Result<Self, FindError> {
        Ok(Self {
            dr: DataRef::find(name.as_ref())?,
            data: PhantomData,
            rust_type: PhantomData,
            conv: PhantomData,
        })
    }
    /// Makes this dataref writable
    ///
    /// Returns an error if the dataref cannot be written.
    pub fn writeable(self) -> Result<TypedDataRef<X, R, C, ReadWrite>, FindError> {
        Ok(TypedDataRef {
            dr: self.dr.writeable()?,
            data: PhantomData,
            rust_type: PhantomData,
            conv: PhantomData,
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::data::typed::{
        borrowed::TypedDataRef, InputUnitConversion, OutputUnitConversion, TypedArrayRead,
        TypedArrayReadWrite, TypedDataRead, TypedDataReadWrite,
    };

    #[test]
    fn test_typed_dataref() {
        let _dr_lock = crate::test_stubs::DATAREF_SYS_LOCK.lock();

        #[derive(Copy, Clone, derive_more::TryFrom, PartialEq, Eq, Debug)]
        #[try_from(repr)]
        #[repr(i32)]
        enum ValidValues {
            A,
            B,
            C,
        }
        struct ValidValuesConv {}
        impl InputUnitConversion<i32, ValidValues> for ValidValuesConv {
            type Error = derive_more::TryFromReprError<i32>;
            fn try_conv_in(value: i32) -> Result<ValidValues, Self::Error> {
                ValidValues::try_from(value)
            }
        }
        impl OutputUnitConversion<ValidValues, i32> for ValidValuesConv {
            fn conv_out(value: &ValidValues) -> i32 {
                *value as _
            }
        }

        let mut dr = TypedDataRef::<i32, ValidValues, ValidValuesConv>::find("test/i32")
            .unwrap()
            .writeable()
            .unwrap();
        let en = ValidValues::C;
        dr.set(en);
        assert_ne!(dr.get().unwrap(), ValidValues::A);
        assert_ne!(dr.get().unwrap(), ValidValues::B);
        assert_eq!(dr.get().unwrap(), ValidValues::C);

        let mut array_dr =
            TypedDataRef::<[i32], ValidValues, ValidValuesConv>::find("test/i32array")
                .unwrap()
                .writeable()
                .unwrap();
        let en = ValidValues::C;
        array_dr.set(&[en]);
        let en_out = array_dr.get_subdata(0..1).unwrap();
        assert_eq!(en_out, vec![ValidValues::C]);
    }
}
