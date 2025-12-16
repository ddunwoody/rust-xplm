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

use super::{PassthruConv, TypedData};

pub type TypedDataRef<X, R, A = ReadOnly, Cin = PassthruConv, Cout = PassthruConv> =
    TypedData<X, R, DataRef<X, A>, Cin, Cout>;

impl<X, R, Cin, Cout> TypedDataRef<X, R, ReadOnly, Cin, Cout>
where
    X: DataType + ?Sized,
{
    pub fn find<S: AsRef<str>>(name: S) -> Result<Self, FindError> {
        Ok(Self {
            dr: DataRef::find(name.as_ref())?,
            data: PhantomData,
            rust_type: PhantomData,
            conv_in: PhantomData,
            conv_out: PhantomData,
        })
    }
    /// Makes this dataref writable
    ///
    /// Returns an error if the dataref cannot be written.
    pub fn writeable(self) -> Result<TypedDataRef<X, R, ReadWrite>, FindError> {
        Ok(TypedDataRef {
            dr: self.dr.writeable()?,
            data: PhantomData,
            rust_type: PhantomData,
            conv_in: PhantomData,
            conv_out: PhantomData,
        })
    }
}
