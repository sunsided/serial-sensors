use std::ops::{Deref, DerefMut};

use array_pool::pool::BorrowingSlice;

/// A slice of data transmitted over the wire.
pub struct DataSlice {
    buffer: BorrowingSlice<u8>,
    size: usize,
}

impl DataSlice {
    pub fn new(buffer: BorrowingSlice<u8>, size: usize) -> Self {
        assert!(buffer.len() >= size);
        Self { buffer, size }
    }

    #[allow(unused)]
    pub fn len(&self) -> usize {
        self.size
    }

    #[allow(unused)]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl AsRef<[u8]> for DataSlice {
    fn as_ref(&self) -> &[u8] {
        &self.buffer[..self.size]
    }
}

impl AsMut<[u8]> for DataSlice {
    fn as_mut(&mut self) -> &mut [u8] {
        &mut self.buffer[..self.size]
    }
}

impl Deref for DataSlice {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        self.as_ref()
    }
}

impl DerefMut for DataSlice {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.as_mut()
    }
}
