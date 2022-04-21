use crate::frame::{util, Error, Frame, Head, Kind, StreamId};
use bytes::{Buf, BufMut, Bytes};

use std::fmt;

/// Bifrost Call frame
///
/// Data frames convey arbitrary, variable-length sequences of octets associated
/// with a stream. One or more DATA frames are used, for instance, to carry HTTP
/// request or response payloads.
#[derive(Eq, PartialEq)]
pub struct BifrostCall<T = Bytes> {
    stream_id: StreamId,
    data: T,
    flags: BifrostCallFlags,
}

#[derive(Copy, Clone, Eq, PartialEq)]
struct BifrostCallFlags(u8);


const NORMAL: u8 = 0x1;
const ONE_SHOOT: u8 = 0x2;
const RESPONSE: u8 = 0x8;
const ALL: u8 = ONE_SHOOT | NORMAL | RESPONSE;

impl<T> BifrostCall<T> {
    /// Creates a new DATA frame.
    pub fn new(stream_id: StreamId, payload: T) -> Self {
        assert!(!stream_id.is_zero());

        BifrostCall {
            stream_id,
            data: payload,
            flags: BifrostCallFlags::default(),
        }
    }

    /// Returns the stream identifier that this frame is associated with.
    ///
    /// This cannot be a zero stream identifier.
    pub fn stream_id(&self) -> StreamId {
        self.stream_id
    }

    /// Returns a reference to this frame's payload.
    ///
    /// This does **not** include any padding that might have been originally
    /// included.
    pub fn payload(&self) -> &T {
        &self.data
    }

    /// Returns a mutable reference to this frame's payload.
    ///
    /// This does **not** include any padding that might have been originally
    /// included.
    pub fn payload_mut(&mut self) -> &mut T {
        &mut self.data
    }

    /// Consumes `self` and returns the frame's payload.
    ///
    /// This does **not** include any padding that might have been originally
    /// included.
    pub fn into_payload(self) -> T {
        self.data
    }

    pub(crate) fn head(&self) -> Head {
        Head::new(Kind::BifrostCall, self.flags.into(), self.stream_id)
    }

    pub fn set_response(&mut self) {
        self.flags.set_response();
    }

    pub(crate) fn map<F, U>(self, f: F) -> BifrostCall<U>
        where
            F: FnOnce(T) -> U,
    {
        BifrostCall {
            stream_id: self.stream_id,
            data: f(self.data),
            flags: self.flags,
        }
    }
}

impl BifrostCall<Bytes> {
    pub(crate) fn load(head: Head, payload: Bytes) -> Result<Self, Error> {
        let flags = BifrostCallFlags::load(head.flag());

        // The stream identifier must not be zero
        if head.stream_id().is_zero() {
            return Err(Error::InvalidStreamId);
        }

        Ok(BifrostCall {
            stream_id: head.stream_id(),
            data: payload,
            flags,
        })
    }
}

impl<T: Buf> BifrostCall<T> {
    /// Encode the data frame into the `dst` buffer.
    ///
    /// # Panics
    ///
    /// Panics if `dst` cannot contain the data frame.
    pub(crate) fn encode_chunk<U: BufMut>(&mut self, dst: &mut U) {
        let len = self.data.remaining() as usize;
        assert!(dst.remaining_mut() >= len);
        self.head().encode(len, dst);
        dst.put(&mut self.data);
    }
}

impl<T> From<BifrostCall<T>> for Frame<T> {
    fn from(src: BifrostCall<T>) -> Self {
        Frame::BifrostCall(src)
    }
}

impl<T> fmt::Debug for BifrostCall<T> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        let mut f = fmt.debug_struct("Data");
        f.field("stream_id", &self.stream_id);
        if !self.flags.is_empty() {
            f.field("flags", &self.flags);
        }
        f.finish()
    }
}

// ===== impl DataFlags =====

impl BifrostCallFlags {
    fn load(bits: u8) -> BifrostCallFlags {
        BifrostCallFlags(bits & ALL)
    }

    fn is_empty(&self) -> bool {
        self.0 == 0
    }

    fn is_one_shoot(&self) -> bool {
        self.0 & ONE_SHOOT == ONE_SHOOT
    }

    fn is_response(&self) -> bool {
        self.0 & RESPONSE == RESPONSE
    }

    fn set_response(&mut self) {
        self.0 |= RESPONSE;
    }

    fn set_one_shoot(&mut self) {
        self.0 |= ONE_SHOOT
    }

    fn unset_one_shoot(&mut self) {
        self.0 &= !ONE_SHOOT
    }

    fn is_normal(&self) -> bool {
        self.0 & NORMAL == NORMAL
    }

    fn set_normal(&mut self) {
        self.0 |= NORMAL
    }
}

impl Default for BifrostCallFlags {
    fn default() -> Self {
        BifrostCallFlags(0)
    }
}

impl From<BifrostCallFlags> for u8 {
    fn from(src: BifrostCallFlags) -> u8 {
        src.0
    }
}

impl fmt::Debug for BifrostCallFlags {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        util::debug_flags(fmt, self.0)
            .flag_if(self.is_one_shoot(), "ONE_SHOOT")
            .flag_if(self.is_normal(), "NORMAL")
            .finish()
    }
}
