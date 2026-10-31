extern crate alloc;
use alloc::boxed::Box;

/// Nonblocking byte transport. `Ok(0)` means no progress; counts must not exceed
/// the supplied slice length. Errors are transport-specific.
pub trait Phy {
    fn send(&mut self, buffer: &[u8]) -> Result<usize, i32>;
    fn receive(&mut self, buffer: &mut [u8]) -> Result<usize, i32>;
}

impl<T: Phy + ?Sized> Phy for Box<T> {
    fn send(&mut self, buffer: &[u8]) -> Result<usize, i32> {
        (**self).send(buffer)
    }

    fn receive(&mut self, buffer: &mut [u8]) -> Result<usize, i32> {
        (**self).receive(buffer)
    }
}

impl<T: Phy + ?Sized> Phy for &mut T {
    fn send(&mut self, buffer: &[u8]) -> Result<usize, i32> {
        (**self).send(buffer)
    }
    fn receive(&mut self, buffer: &mut [u8]) -> Result<usize, i32> {
        (**self).receive(buffer)
    }
}
