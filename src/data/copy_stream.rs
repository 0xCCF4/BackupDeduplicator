use std::cell::RefCell;
use std::io::Read;
use std::rc::Rc;
use anyhow::Result;

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
        let reader = Rc::try_unwrap(self.reader).map(|x| x.into_inner()).map_err(|_| anyhow::anyhow!("Could not unwrap reader."))?;
        let buffer = Rc::try_unwrap(self.buffer).map(|x| x.into_inner()).map_err(|_| anyhow::anyhow!("Could not unwrap buffer."))?;
        
        Ok(BufferFirstContinueReader::new(reader, buffer))
    }

    /// Create a new child stream that will read/copying the data from the original stream.
    ///
    /// # Returns
    /// A new [BufferCopyStreamReaderChild] instance.
    pub fn child(&self) -> BufferCopyStreamReaderChild<R> {
        BufferCopyStreamReaderChild::new(Rc::clone(&self.buffer), Rc::clone(&self.reader))
    }
}

/// A child stream that reads from the [BufferCopyStreamReader] while buffering the data.
///
/// # Example
/// See [BufferCopyStreamReader] for an example.
pub struct BufferCopyStreamReaderChild<R: Read> {
    buffer: Rc<RefCell<Vec<u8>>>,
    reader: Rc<RefCell<R>>,
    index: usize,
}

impl<R: Read> BufferCopyStreamReaderChild<R> {
    fn new(buffer: Rc<RefCell<Vec<u8>>>, reader: Rc<RefCell<R>>) -> Self {
        BufferCopyStreamReaderChild {
            buffer,
            reader,
            index: 0,
        }
    }

    /// Buffer the given amount of bytes from the underlying reader.
    ///
    /// # Arguments
    /// * `length` - The amount of bytes to buffer.
    ///
    /// # Returns
    /// The amount of bytes read.
    ///
    /// # Errors
    /// If the underlying reader could not be read.
    pub fn buffer_bytes(&self, length: usize) -> std::io::Result<usize> {
        let mut buffer = self.buffer.borrow_mut();
        let mut reader = self.reader.borrow_mut();

        let mut tmp_buffer = Vec::with_capacity(length);
        for _ in 0..length {
            tmp_buffer.push(0);
        }

        let read_result = reader.read(&mut tmp_buffer)?;

        buffer.reserve(read_result);
        for i in 0..read_result {
            buffer.push(tmp_buffer[i]);
        }

        Ok(read_result)
    }
}

impl<R: Read> Read for BufferCopyStreamReaderChild<R> {
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
        self.buffer_bytes(requested_bytes)?;
        let buffer = self.buffer.borrow();

        let bytes_to_copy = std::cmp::min(buffer.len() - self.index, buf.len());
        
        // copy bytes from buffer to buf
        buf[..bytes_to_copy].copy_from_slice(&buffer[self.index..self.index + bytes_to_copy]);
        
        Ok(bytes_to_copy)
    }
}

/// A [Read] stream that first reads from a buffer before continuing to read from the underlying stream.
///
/// # Example
/// See [BufferCopyStreamReader] for an example.
pub struct BufferFirstContinueReader<R: Read> {
    reader: R,
    buffer: Vec<u8>,
    index: usize
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
        self.buffer.len() <= 0 || self.index >= self.buffer.len()
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
            buf[0..buffered_bytes].copy_from_slice(&self.buffer[self.index..self.index + buffered_bytes]);
            self.index += buffered_bytes;
        }

        if self.buffer_empty() {
            self.index = 0;
            self.buffer.clear();
        }

        if requested_bytes > 0 {
            let read_bytes = self.reader.read(&mut buf[buffered_bytes..buffered_bytes + requested_bytes])?;
            Ok(buffered_bytes + read_bytes)
        } else {
            Ok(buffered_bytes)
        }
    }
}
