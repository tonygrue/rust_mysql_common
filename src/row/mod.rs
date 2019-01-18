// Copyright (c) 2017 Anatoly Ikorsky
//
// Licensed under the Apache License, Version 2.0
// <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0> or the MIT
// license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. All files in the project carrying such notice may not be copied,
// modified, or distributed except according to those terms.

use crate::packets::Column;
use crate::value::convert::{from_value, from_value_opt, FromValue, FromValueError};
use crate::value::Value;
use smallvec::SmallVec;
use std::fmt;
use std::ops::Index;
use std::sync::Arc;

pub mod convert;

/// Client side representation of a MySql row.
///
/// It allows you to move column values out of a row with `Row::take` method but note that it
/// makes row incomplete. Calls to `from_row_opt` on incomplete row will return
/// `Error::FromRowError` and also numerical indexing on taken columns will panic.
#[derive(Clone, PartialEq)]
pub struct Row {
    values: SmallVec<[Option<Value>; 12]>,
    columns: Arc<Vec<Column>>,
}

impl fmt::Debug for Row {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut debug = f.debug_struct("Row");
        for (val, column) in self.values.iter().zip(self.columns.iter()) {
            match *val {
                Some(ref val) => {
                    debug.field(column.name_str().as_ref(), val);
                }
                None => {
                    debug.field(column.name_str().as_ref(), &"<taken>");
                }
            }
        }
        debug.finish()
    }
}

/// Creates `Row` from values and columns.
pub fn new_row(values: SmallVec<[Value; 12]>, columns: Arc<Vec<Column>>) -> Row {
    assert!(values.len() == columns.len());
    Row {
        values: values.into_iter().map(|value| Some(value)).collect(),
        columns,
    }
}

impl Row {
    /// Returns length of a row.
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// Returns columns of this row.
    pub fn columns_ref(&self) -> &[Column] {
        &**self.columns
    }

    /// Returns columns of this row.
    pub fn columns(&self) -> Arc<Vec<Column>> {
        self.columns.clone()
    }

    /// Returns reference to the value of a column with index `index` if it exists and wasn't taken
    /// by `Row::take` method.
    ///
    /// Non panicking version of `row[usize]`.
    pub fn as_ref(&self, index: usize) -> Option<&Value> {
        self.values.get(index).and_then(|x| x.as_ref())
    }

    /// Will copy value at index `index` if it was not taken by `Row::take` earlier,
    /// then will convert it to `T`.
    pub fn get<T, I>(&self, index: I) -> Option<T>
    where
        T: FromValue,
        I: ColumnIndex,
    {
        index.idx(&*self.columns).and_then(|idx| {
            self.values
                .get(idx)
                .and_then(|x| x.as_ref())
                .map(|x| from_value::<T>(x.clone()))
        })
    }

    /// Will copy value at index `index` if it was not taken by `Row::take` or `Row::take_opt`
    /// earlier, then will attempt convert it to `T`. Unlike `Row::get`, `Row::get_opt` will
    /// allow you to directly handle errors if the value could not be converted to `T`.
    pub fn get_opt<T, I>(&self, index: I) -> Option<Result<T, FromValueError>>
    where
        T: FromValue,
        I: ColumnIndex,
    {
        index
            .idx(&*self.columns)
            .and_then(|idx| self.values.get(idx))
            .and_then(|x| x.as_ref())
            .and_then(|x| Some(from_value_opt::<T>(x.clone())))
    }

    /// Will take value of a column with index `index` if it exists and wasn't taken earlier then
    /// will converts it to `T`.
    pub fn take<T, I>(&mut self, index: I) -> Option<T>
    where
        T: FromValue,
        I: ColumnIndex,
    {
        index.idx(&*self.columns).and_then(|idx| {
            self.values
                .get_mut(idx)
                .and_then(|x| x.take())
                .map(from_value::<T>)
        })
    }

    /// Will take value of a column with index `index` if it exists and wasn't taken earlier then
    /// will attempt to convert it to `T`. Unlike `Row::take`, `Row::take_opt` will allow you to
    /// directly handle errors if the value could not be converted to `T`.
    pub fn take_opt<T, I>(&mut self, index: I) -> Option<Result<T, FromValueError>>
    where
        T: FromValue,
        I: ColumnIndex,
    {
        index
            .idx(&*self.columns)
            .and_then(|idx| self.values.get_mut(idx))
            .and_then(|x| x.take())
            .and_then(|x| Some(from_value_opt::<T>(x)))
    }

    /// Unwraps values of a row.
    ///
    /// # Panics
    ///
    /// Panics if any of columns was taken by `take` method.
    pub fn unwrap(self) -> Vec<Value> {
        self.values
            .into_iter()
            .map(|x| x.expect("Can't unwrap row if some of columns was taken"))
            .collect()
    }

    #[doc(hidden)]
    pub fn place(&mut self, index: usize, value: Value) {
        self.values[index] = Some(value);
    }
}

impl Index<usize> for Row {
    type Output = Value;

    fn index<'a>(&'a self, index: usize) -> &'a Value {
        self.values[index].as_ref().unwrap()
    }
}

impl<'a> Index<&'a str> for Row {
    type Output = Value;

    fn index<'r>(&'r self, index: &'a str) -> &'r Value {
        for (i, column) in self.columns.iter().enumerate() {
            if column.name_ref() == index.as_bytes() {
                return self.values[i].as_ref().unwrap();
            }
        }
        panic!("No such column: `{}` in row {:?}", index, self);
    }
}

/// Things that may be used as an index of a row column.
pub trait ColumnIndex {
    fn idx(&self, columns: &Vec<Column>) -> Option<usize>;
}

impl ColumnIndex for usize {
    fn idx(&self, columns: &Vec<Column>) -> Option<usize> {
        if *self >= columns.len() {
            None
        } else {
            Some(*self)
        }
    }
}

impl<'a> ColumnIndex for &'a str {
    fn idx(&self, columns: &Vec<Column>) -> Option<usize> {
        for (i, c) in columns.iter().enumerate() {
            if c.name_ref() == self.as_bytes() {
                return Some(i);
            }
        }
        None
    }
}
