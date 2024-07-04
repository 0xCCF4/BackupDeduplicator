use anyhow::Result;
use std::cell::RefCell;
use std::cmp::min;
use std::io::{ErrorKind, Read, Seek, SeekFrom};
use std::rc::Rc;

/// A [Read] stream wrapper that allows creating multiple child streams that will
/// read from the same underlying stream while buffering the data.
/// This allows for multiple streams to read the same data by only reading it once
/// from the underlying stream.
///
/// # Example
/// ```
/// # use std::io::{Cursor, Read};
/// # use backup_deduplicator::copy_stream::BufferCopyStreamReader;
/// #
/// # let mut data = vec![];
/// # for i in 0..100 {
/// #     data.push(i);
/// # }
/// # let data = Cursor::new(data);
/// #
/// // data is the underlying reader
/// let main_reader = BufferCopyStreamReader::new(data);
///
/// for _ in 0..10 {
///     let mut buffer = [0; 10];
///     let mut reader = main_reader.child();
///
///     reader.read(&mut buffer).unwrap();
///
///     assert_eq!(buffer, [0, 1, 2, 3, 4, 5, 6, 7, 8, 9]);
/// }
///
/// // finish session and get back the original reader
///
/// // read all buffered bytes
/// let mut reader = main_reader.try_into_inner().unwrap();
/// let mut buffer = [0; 10];
/// reader.read(&mut buffer).unwrap();
///
/// // get back original data reader
/// let mut data = reader.try_into_inner().unwrap();
/// ```
pub struct BufferCopyStreamReader<R: Read> {
    reader: Rc<RefCell<R>>,
    buffer: Rc<RefCell<Vec<u8>>>,
    index: usize,
}

impl<R: Read> BufferCopyStreamReader<R> {
    /// Create a new [BufferCopyStreamReader] instance with the given reader.
    ///
    /// # Arguments
    /// * `reader` - The reader to wrap.
    ///
    /// # Returns
    /// A new [BufferCopyStreamReader] instance.
    pub fn new(reader: R) -> Self {
        BufferCopyStreamReader {
            reader: Rc::new(RefCell::new(reader)),
            buffer: Rc::new(RefCell::new(Vec::new())),
            index: 0,
        }
    }

    /// Create a new [BufferCopyStreamReader] instance with the given reader and buffer capacity.
    /// Reallocating the buffer with the given capacity.
    ///
    /// # Arguments
    /// * `reader` - The reader to wrap.
    /// * `capacity` - The capacity of the buffer.
    ///
    /// # Returns
    /// A new [BufferCopyStreamReader] instance.
    pub fn with_capacity(reader: R, capacity: usize) -> Self {
        BufferCopyStreamReader {
            reader: Rc::new(RefCell::new(reader)),
            buffer: Rc::new(RefCell::new(Vec::with_capacity(capacity))),
            index: 0,
        }
    }

    /// Create a new [BufferCopyStreamReader] instance with the given reader and no initial buffer.
    ///
    /// # Arguments
    /// * `reader` - The reader to wrap.
    ///
    /// # Returns
    /// A new [BufferCopyStreamReader] instance.
    pub fn with_no_capacity(reader: R) -> Self {
        Self::with_capacity(reader, 0)
    }

    /// Try to get the original reader back from the [BufferCopyStreamReader].
    /// The resulting reader will first provide the buffered bytes before continue
    /// reading from the original reader.
    ///
    /// # Returns
    /// The original reader wrapped in a [BufferFirstContinueReader].
    ///
    /// # Errors
    /// If there are still child stream open.
    pub fn try_into_inner(self) -> Result<BufferFirstContinueReader<R>> {
        let reader = Rc::try_unwrap(self.reader)
            .map(|x| x.into_inner())
            .map_err(|_| anyhow::anyhow!("Could not unwrap reader."))?;
        let buffer = Rc::try_unwrap(self.buffer)
            .map(|x| x.into_inner())
            .map_err(|_| anyhow::anyhow!("Could not unwrap buffer."))?;

        Ok(BufferFirstContinueReader::new(reader, buffer))
    }

    /// Create a new child stream that will read/copying the data from the original stream.
    ///
    /// # Returns
    /// A new [BufferCopyStreamReaderChild] instance.
    pub fn child(&self) -> BufferCopyStreamReader<R> {
        BufferCopyStreamReader {
            reader: Rc::clone(&self.reader),
            buffer: Rc::clone(&self.buffer),
            index: self.index,
        }
    }

    /// Buffer the given amount of bytes from the underlying reader.
    ///
    /// # Arguments
    /// * `length` - The amount of bytes to buffer.
    /// * `read_buffer` - A buffer used for temporary storage. If none method will allocate own buffer.
    ///
    /// # Returns
    /// The amount of bytes read.
    ///
    /// # Errors
    /// If the underlying reader could not be read.
    pub fn buffer_bytes(
        &self,
        length: usize,
        read_buffer: Option<&mut Vec<u8>>,
    ) -> std::io::Result<usize> {
        fn inner<R: Read>(
            this: &BufferCopyStreamReader<R>,
            length: usize,
            read_buffer: &mut Vec<u8>,
        ) -> std::io::Result<usize> {
            let mut buffer = this.buffer.borrow_mut();
            let mut reader = this.reader.borrow_mut();

            while read_buffer.len() < length {
                read_buffer.push(0);
            }

            let window = &mut read_buffer[..length];

            let read_result = reader.read(window)?;

            buffer.reserve(read_result);
            for value in window.iter().take(read_result) {
                buffer.push(*value);
            }

            Ok(read_result)
        }

        match read_buffer {
            Some(read_buffer) => inner(self, length, read_buffer),
            None => {
                let mut read_buffer: Vec<u8> = Vec::with_capacity(length);
                inner(self, length, &mut read_buffer)
            }
        }
    }

    /// Buffer the given amount of bytes from the underlying reader in chunks.
    /// This method will allocate a buffer with the given size once. Then reuse this buffer
    /// for reading the data from the underlying reader.
    ///
    /// # Arguments
    /// * `length` - The amount of bytes to buffer.
    /// * `chunk_size` - The amount of bytes to request from the underlying reader in each iteration.
    ///
    /// # Returns
    /// The amount of bytes read.
    ///
    /// # Errors
    /// If the underlying reader could not be read.
    pub fn buffer_bytes_chunked(&self, length: usize, chunk_size: usize) -> std::io::Result<usize> {
        let mut allocated = 0;

        // only allocate temporary buffer once, then reuse
        let mut read_buffer = Vec::with_capacity(min(length, chunk_size));

        loop {
            let bytes_to_read = min(length - allocated, chunk_size);

            // required for streams that do not have an EOF
            if bytes_to_read == 0 {
                return Ok(allocated);
            }

            match self.buffer_bytes(bytes_to_read, Some(&mut read_buffer))? {
                0 => return Ok(allocated),
                bytes_read => allocated += bytes_read,
            }
        }
    }

    /// Buffer the given amount of bytes from the underlying reader in chunks.
    /// Uses a default chunk size of 4096 bytes.
    ///
    /// # Arguments
    /// * `length` - The amount of bytes to buffer.
    ///
    /// # Returns
    /// The amount of bytes read.
    ///
    /// # Errors
    /// If the underlying reader could not be read.
    pub fn buffer_bytes_chunked_default(&self, length: usize) -> std::io::Result<usize> {
        self.buffer_bytes_chunked(length, 4096)
    }
}

impl<R: Read> Read for BufferCopyStreamReader<R> {
    /// Read bytes from the buffer by silently buffering the data from the underlying reader.
    ///
    /// # Arguments
    /// * `buf` - The buffer to read into.
    ///
    /// # Returns
    /// The amount of bytes read.
    ///
    /// # Errors
    /// If the underlying reader could not be read.
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let buffer = self.buffer.borrow();

        let buffered_bytes = std::cmp::min(buffer.len() - self.index, buf.len());
        let requested_bytes = std::cmp::max(0, buf.len() - buffered_bytes);

        drop(buffer);
        self.buffer_bytes_chunked_default(requested_bytes)?;
        let buffer = self.buffer.borrow();

        let bytes_to_copy = std::cmp::min(buffer.len() - self.index, buf.len());

        // copy bytes from buffer to buf
        buf[..bytes_to_copy].copy_from_slice(&buffer[self.index..self.index + bytes_to_copy]);

        Ok(bytes_to_copy)
    }
}

impl<R: Read> Seek for BufferCopyStreamReader<R> {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        let target_index = match pos {
            SeekFrom::Start(requested_position) => {
                // get last index
                let buffer = self.buffer.borrow();
                let buffer_length = buffer.len() as u64;
                drop(buffer);

                // only check upper bound, since unsigned parameter
                if requested_position > buffer_length {
                    self.buffer_bytes_chunked_default(
                        (requested_position - buffer_length) as usize,
                    )?;
                }

                requested_position
            }
            SeekFrom::End(requested_position) => {
                // buffer all bytes till end
                loop {
                    match self.buffer_bytes_chunked_default(usize::MAX) {
                        Err(e) => return Err(e),
                        Ok(0) => break,
                        Ok(_) => continue,
                    }
                }

                // get last index
                let buffer = self.buffer.borrow();
                let buffer_length = buffer.len() as u64;

                // check upper and lower bound
                if requested_position > 0 {
                    return Err(std::io::Error::new(
                        ErrorKind::Other,
                        "Can not seek beyond end",
                    ));
                }

                if (-requested_position as u64) > buffer_length {
                    return Err(std::io::Error::new(
                        ErrorKind::Other,
                        "can not seek beyond zero",
                    ));
                }

                buffer_length - (-requested_position as u64)
            }
            SeekFrom::Current(requested_position) => {
                return if requested_position >= 0 {
                    if (self.index as u64)
                        .checked_add(requested_position as u64)
                        .is_none()
                    {
                        return Err(std::io::Error::new(
                            ErrorKind::Other,
                            "Can not seek beyond overflow.",
                        ));
                    }

                    self.seek(SeekFrom::Start(
                        self.index as u64 + requested_position as u64,
                    ))
                } else {
                    if (self.index as u64) < (-requested_position as u64) {
                        return Err(std::io::Error::new(
                            ErrorKind::Other,
                            "can not seek beyond zero",
                        ));
                    }

                    self.seek(SeekFrom::Start(
                        self.index as u64 - (-requested_position as u64),
                    ))
                }
            }
        };

        self.index = target_index as usize;
        Ok(self.index as u64)
    }
}

/// A [Read] stream that first reads from a buffer before continuing to read from the underlying stream.
///
/// # Example
/// See [BufferCopyStreamReader] for an example.
pub struct BufferFirstContinueReader<R: Read> {
    reader: R,
    buffer: Vec<u8>,
    index: usize,
}

impl<R: Read> BufferFirstContinueReader<R> {
    /// Create a new [BufferFirstContinueReader] instance with the given reader and buffer.
    ///
    /// # Arguments
    /// * `reader` - The reader to wrap.
    /// * `buffer` - The buffer to read from first.
    ///
    /// # Returns
    /// A new [BufferFirstContinueReader] instance.
    pub fn new(reader: R, buffer: Vec<u8>) -> BufferFirstContinueReader<R> {
        BufferFirstContinueReader {
            reader,
            buffer,
            index: 0,
        }
    }

    #[allow(dead_code)]
    fn with_index(reader: R, mut buffer: Vec<u8>, index: usize) -> BufferFirstContinueReader<R> {
        let index = if index >= buffer.len() {
            buffer.clear();
            buffer.shrink_to_fit();
            0
        } else {
            index
        };
        BufferFirstContinueReader {
            reader,
            buffer,
            index,
        }
    }

    /// Get the amount of bytes left in the buffer.
    ///
    /// # Returns
    /// The amount of bytes left in the buffer.
    pub fn left_over_bytes(&self) -> usize {
        self.buffer.len() - self.index
    }

    /// Check if the buffer is empty.
    ///
    /// # Returns
    /// True if the buffer is empty.
    pub fn buffer_empty(&self) -> bool {
        self.buffer.is_empty() || self.index >= self.buffer.len()
    }

    /// Try to get the original reader back from the [BufferFirstContinueReader].
    ///
    /// # Returns
    /// The original reader.
    ///
    /// # Errors
    /// If there are still bytes left in the buffer. Check [BufferFirstContinueReader::left_over_bytes] for the number
    /// of bytes left in the buffer.
    pub fn try_into_inner(self) -> Result<R> {
        if self.buffer_empty() {
            Ok(self.reader)
        } else {
            Err(anyhow::anyhow!("Buffer not empty. Still data to read."))
        }
    }
}

impl<R: Read> Read for BufferFirstContinueReader<R> {
    /// Read bytes from the buffer first before continuing to read from the underlying reader.
    ///
    /// # Arguments
    /// * `buf` - The buffer to read into.
    ///
    /// # Returns
    /// The amount of bytes read.
    ///
    /// # Errors
    /// If the underlying reader could not be read.
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let buffered_bytes = std::cmp::min(self.buffer.len() - self.index, buf.len());
        let requested_bytes = std::cmp::max(0, buf.len() - buffered_bytes);

        if buffered_bytes > 0 {
            buf[0..buffered_bytes]
                .copy_from_slice(&self.buffer[self.index..self.index + buffered_bytes]);
            self.index += buffered_bytes;
        }

        if self.buffer_empty() {
            self.index = 0;
            self.buffer.clear();
            self.buffer.shrink_to_fit();
        }

        if requested_bytes > 0 {
            let read_bytes = self
                .reader
                .read(&mut buf[buffered_bytes..buffered_bytes + requested_bytes])?;
            Ok(buffered_bytes + read_bytes)
        } else {
            Ok(buffered_bytes)
        }
    }
}

impl<T: Read> From<T> for BufferCopyStreamReader<T> {
    fn from(reader: T) -> Self {
        BufferCopyStreamReader::new(reader)
    }
}
