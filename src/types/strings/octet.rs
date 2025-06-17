use crate::{
    macros::{constraints, size_constraint},
    prelude::*,
};

use alloc::vec::Vec;

cfg_if::cfg_if! {
    if #[cfg(all(feature = "arc-slice", feature = "bytes"))] {
        compile_error!("features `arc-slice` and `bytes` conflict, choose one");
    } else if #[cfg(feature = "bytes")] {
        type BytesImpl = bytes::Bytes;
        pub(crate) type DecodedOctetString = Vec<u8>;
    } else if #[cfg(feature = "arc-slice")] {
        type BytesImpl = arc_slice::ArcBytes<arc_slice::layout::ArcLayout<true, true>>;
        pub(crate) type DecodedOctetString = arc_slice_impl::OctetStringWrapper;
    } else {
        compile_error!("feature `bytes` or `arc-slice` must be enabled");
    }
}

/// The `OCTET STRING` type.
///
/// This type represents a contiguous sequence of bytes. It is the ASN.1
/// equivalent to `Vec<u8>`. Rasn uses a specialised type to allow for `Vec<T>`
/// to represent `SEQUENCE OF`, and to provide optimisations specific to bytes.
///
/// This type should be considered cheaply clonable.
///
/// ```
/// use rasn::types::OctetString;
///
/// let os = OctetString::from(vec![0xDE, 0xAD, 0xBE, 0xEF]);
///
/// for byte in &*os {
///    println!("{byte:0x}");
/// }
/// ```
/// ### Feature Flags
/// You can enable an alternative container implementation `arc-slice`, this
/// should provide roughly 7–10% performance increase, but is newer and less
/// well tested than the default `bytes` implementation.
#[derive(Debug, Default, Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct OctetString(BytesImpl);

impl OctetString {
    /// Creates a new [OctetString] from a static slice.
    ///
    /// The returned [OctetString] will point directly to the static slice.
    /// There is no allocating or copying.
    pub const fn from_static(value: &'static [u8]) -> Self {
        Self(BytesImpl::from_static(value))
    }

    /// Creates a new [OctetString] from a slice by copying it.
    pub fn from_slice(value: &[u8]) -> Self {
        Self::from(value)
    }
}

impl AsRef<[u8]> for OctetString {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl<const N: usize> From<[u8; N]> for OctetString {
    fn from(value: [u8; N]) -> Self {
        cfg_if::cfg_if! {
            if #[cfg(feature = "arc-slice")] {
                Self(value.into())
            } else {
                Self(bytes::Bytes::copy_from_slice(&value))
            }
        }
    }
}

impl From<Vec<u8>> for OctetString {
    fn from(value: Vec<u8>) -> Self {
        Self(value.into())
    }
}

impl From<&[u8]> for OctetString {
    fn from(value: &[u8]) -> Self {
        cfg_if::cfg_if! {
            if #[cfg(feature = "arc-slice")] {
                Self(value.into())
            } else {
                Self(bytes::Bytes::copy_from_slice(value))
            }
        }
    }
}

impl core::ops::Deref for OctetString {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl PartialEq<[u8]> for OctetString {
    fn eq(&self, value: &[u8]) -> bool {
        self.0 == value
    }
}

impl PartialEq<&[u8]> for OctetString {
    fn eq(&self, value: &&[u8]) -> bool {
        &self.0 == value
    }
}

impl PartialEq<Vec<u8>> for OctetString {
    fn eq(&self, value: &Vec<u8>) -> bool {
        self.0 == *value
    }
}

/// A buffer that can be used to efficiently decode an `OCTET STRING`.
pub trait OctetStringBuffer: Default + FromIterator<u8> {
    /// Extracts the buffered slice.
    fn as_slice(&self) -> &[u8];
    /// Appends the bytes to the buffer.
    fn extend_from_slice(&mut self, value: &[u8]);
    /// Creates a buffer from a bytes slice.
    fn from_slice(value: &[u8]) -> Self;
}

impl OctetStringBuffer for Vec<u8> {
    fn as_slice(&self) -> &[u8] {
        self
    }

    fn extend_from_slice(&mut self, value: &[u8]) {
        self.extend_from_slice(value);
    }

    fn from_slice(value: &[u8]) -> Self {
        value.to_vec()
    }
}

/// Conversion trait used when decoding an `OCTET STRING`.
///
/// It is used as a trait bound in [`Decoder::decode_octet_string`].
pub trait FromOctetString<'a> {
    /// The buffer used for `OCTET STRING` decoding when needed.
    type Buffer: OctetStringBuffer;
    /// Build the object directly from a bytes slice when the `OCTET STRING`
    /// is contiguous and properly aligned.
    fn from_slice(slice: &'a [u8]) -> Self;
    /// Build the object from the buffer used to decode the `OCTET STRING`
    /// when is not contiguous or properly aligned.  
    fn from_buffer(buffer: Self::Buffer) -> Self;
    /// Build the object directly from a bytes vector when it is already
    /// decoded into a vector.
    fn from_vec(vec: Vec<u8>) -> Self;
}

impl<'a, T: From<&'a [u8]> + From<Vec<u8>>> FromOctetString<'a> for T {
    type Buffer = Vec<u8>;
    fn from_slice(slice: &'a [u8]) -> Self {
        T::from(slice)
    }
    fn from_buffer(buffer: Self::Buffer) -> Self {
        T::from(buffer)
    }
    fn from_vec(vec: Vec<u8>) -> Self {
        T::from(vec)
    }
}

#[cfg(feature = "arc-slice")]
mod arc_slice_impl {
    use super::{FromOctetString, OctetString, OctetStringBuffer};
    use alloc::vec::Vec;

    #[derive(Default)]
    pub(crate) struct OctetStringBufferImpl(
        arc_slice::ArcBytesMut<arc_slice::layout::ArcLayout<false, false>>,
    );

    impl OctetStringBuffer for OctetStringBufferImpl {
        fn as_slice(&self) -> &[u8] {
            &self.0
        }

        fn extend_from_slice(&mut self, value: &[u8]) {
            self.0.extend_from_slice(value);
        }

        fn from_slice(value: &[u8]) -> Self {
            Self(value.into())
        }
    }

    impl FromIterator<u8> for OctetStringBufferImpl {
        fn from_iter<T: IntoIterator<Item = u8>>(iter: T) -> Self {
            Self(iter.into_iter().collect())
        }
    }

    pub(crate) struct OctetStringWrapper(OctetString);

    impl FromOctetString<'_> for OctetStringWrapper {
        type Buffer = OctetStringBufferImpl;
        fn from_slice(slice: &[u8]) -> Self {
            Self(slice.into())
        }
        fn from_buffer(buffer: Self::Buffer) -> Self {
            Self(OctetString(buffer.0.freeze()))
        }
        fn from_vec(vec: Vec<u8>) -> Self {
            Self(vec.into())
        }
    }

    impl From<OctetStringWrapper> for OctetString {
        fn from(value: OctetStringWrapper) -> Self {
            value.0
        }
    }
}

/// An `OCTET STRING` which has a fixed size range. This type uses const
/// generics to be able to place the octet string on the stack rather than the
/// heap.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FixedOctetString<const N: usize>([u8; N]);

impl<const N: usize> FixedOctetString<N> {
    /// Creates a new octet string from a given array.
    pub fn new(value: [u8; N]) -> Self {
        Self(value)
    }
}

impl<const N: usize> From<[u8; N]> for FixedOctetString<N> {
    fn from(value: [u8; N]) -> Self {
        Self::new(value)
    }
}

impl<const N: usize> TryFrom<Vec<u8>> for FixedOctetString<N> {
    type Error = <[u8; N] as TryFrom<Vec<u8>>>::Error;

    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
        value.try_into().map(Self)
    }
}

impl<const N: usize> TryFrom<&[u8]> for FixedOctetString<N> {
    type Error = <[u8; N] as TryFrom<&'static [u8]>>::Error;

    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        value.try_into().map(Self)
    }
}

impl<const N: usize> TryFrom<OctetString> for FixedOctetString<N> {
    type Error = <[u8; N] as TryFrom<&'static [u8]>>::Error;

    fn try_from(value: OctetString) -> Result<Self, Self::Error> {
        (&*value).try_into().map(Self)
    }
}

impl<const N: usize> core::ops::Deref for FixedOctetString<N> {
    type Target = [u8; N];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<const N: usize> core::ops::DerefMut for FixedOctetString<N> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<const N: usize> AsnType for FixedOctetString<N> {
    const TAG: Tag = Tag::OCTET_STRING;
    const CONSTRAINTS: Constraints = constraints!(size_constraint!(N));
    const IDENTIFIER: Identifier = Identifier::OCTET_STRING;
}

impl<const N: usize> Decode for FixedOctetString<N> {
    fn decode_with_tag_and_constraints<D: Decoder>(
        decoder: &mut D,
        tag: Tag,
        constraints: Constraints,
    ) -> Result<Self, D::Error> {
        let codec = decoder.codec();
        let bytes = decoder.decode_octet_string::<alloc::borrow::Cow<[u8]>>(tag, constraints)?;
        let len = bytes.len();
        match bytes {
            alloc::borrow::Cow::Borrowed(slice) => Self::try_from(slice).map_err(|_| {
                D::Error::from(crate::error::DecodeError::fixed_string_conversion_failed(
                    tag,
                    bytes.len(),
                    N,
                    codec,
                ))
            }),
            alloc::borrow::Cow::Owned(vec) => Self::try_from(vec).map_err(|_| {
                D::Error::from(crate::error::DecodeError::fixed_string_conversion_failed(
                    tag, len, N, codec,
                ))
            }),
        }
    }
}

impl<const N: usize> Encode for FixedOctetString<N> {
    fn encode_with_tag_and_constraints<'b, E: Encoder<'b>>(
        &self,
        encoder: &mut E,
        tag: Tag,
        constraints: Constraints,
        identifier: Identifier,
    ) -> Result<(), E::Error> {
        encoder
            .encode_octet_string(tag, constraints, &self.0, identifier)
            .map(drop)
    }
}
