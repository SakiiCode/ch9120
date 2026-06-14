use core::ops::{Deref, DerefMut};

use embassy_futures::select::{Either, select};
use embassy_time::{Duration, Timer};
use embedded_io_async::{ErrorType, Read, Write};

pub struct TimeoutBuffer<T: Read + Write> {
    inner: T,
    timeout: Duration,
}

impl<T: Read + Write> ErrorType for TimeoutBuffer<T> {
    type Error = T::Error;
}

impl<T: Read + Write> TimeoutBuffer<T> {
    pub fn new(inner: T, timeout: Duration) -> Self {
        Self { inner, timeout }
    }
    pub fn inner(&mut self) -> &mut T {
        &mut self.inner
    }
}

impl<T: Read + Write> Deref for TimeoutBuffer<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<T: Read + Write> DerefMut for TimeoutBuffer<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

impl<T: Read + Write> Read for TimeoutBuffer<T> {
    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        let mut finished = false;
        let mut idx = 0;
        while !finished {
            let mut internal_buffer = [0u8; 256];
            let timeout_future = Timer::after(self.timeout);
            let read_future = self.inner.read(&mut internal_buffer);
            match select(read_future, timeout_future).await {
                Either::First(result) => {
                    let bytes = result?;
                    #[cfg(feature = "defmt")]
                    defmt::trace!("Read {} bytes", bytes);
                    buf[idx..idx + bytes].copy_from_slice(&internal_buffer[0..bytes]);
                    idx += bytes;
                }
                Either::Second(_) => {
                    finished = true;
                }
            }
        }
        Ok(idx)
    }

    async fn read_exact(
        &mut self,
        buf: &mut [u8],
    ) -> Result<(), embedded_io_async::ReadExactError<Self::Error>> {
        self.inner.read_exact(buf).await
    }
}

impl<T: Read + Write> Write for TimeoutBuffer<T> {
    fn flush(&mut self) -> impl Future<Output = Result<(), Self::Error>> {
        self.inner.flush()
    }

    fn write(&mut self, buf: &[u8]) -> impl Future<Output = Result<usize, Self::Error>> {
        self.inner.write(buf)
    }

    fn write_all(&mut self, buf: &[u8]) -> impl Future<Output = Result<(), Self::Error>> {
        self.inner.write_all(buf)
    }
}
