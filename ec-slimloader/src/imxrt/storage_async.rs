use embedded_storage::nor_flash as es;
use embedded_storage_async::nor_flash as esa;

/// Wrapper for anything that implements the traits from [embedded_storage_async::nor_flash]
/// such that they implement the traits from [embedded_storage::nor_flash].
pub struct AsyncWrapper<T>(pub T);

impl<T: es::ErrorType> esa::ErrorType for AsyncWrapper<T> {
    type Error = T::Error;
}

impl<T: es::ReadNorFlash> esa::ReadNorFlash for AsyncWrapper<T> {
    const READ_SIZE: usize = T::READ_SIZE;

    async fn read(&mut self, offset: u32, bytes: &mut [u8]) -> Result<(), Self::Error> {
        self.0.read(offset, bytes)
    }

    fn capacity(&self) -> usize {
        self.0.capacity()
    }
}

impl<T: es::NorFlash> esa::NorFlash for AsyncWrapper<T> {
    const WRITE_SIZE: usize = T::WRITE_SIZE;

    const ERASE_SIZE: usize = T::ERASE_SIZE;

    async fn erase(&mut self, from: u32, to: u32) -> Result<(), Self::Error> {
        self.0.erase(from, to)
    }

    async fn write(&mut self, offset: u32, bytes: &[u8]) -> Result<(), Self::Error> {
        self.0.write(offset, bytes)
    }
}
