// Copyright 2018-2020 Cargill Incorporated
// Copyright 2018 Bitwise IO, Inc.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

#[derive(Debug)]
pub enum ProtoConversionError {
    DeserializationError(String),
    SerializationError(String),
    InvalidTypeError(String),
}

impl std::error::Error for ProtoConversionError {}

impl std::fmt::Display for ProtoConversionError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match *self {
            ProtoConversionError::DeserializationError(ref s) => {
                write!(f, "unable to deserialize during protobuf conversion: {}", s)
            }
            ProtoConversionError::SerializationError(ref s) => {
                write!(f, "unable to serialize during protobuf conversion: {}", s)
            }
            ProtoConversionError::InvalidTypeError(ref s) => write!(
                f,
                "invalid type encountered during protobuf conversion: {}",
                s
            ),
        }
    }
}

pub trait FromProto<P>: Sized {
    fn from_proto(other: P) -> Result<Self, ProtoConversionError>;
}

pub trait FromNative<N>: Sized {
    fn from_native(other: N) -> Result<Self, ProtoConversionError>;
}

/// Convert from bytes into a native struct, via the given protocol.
pub trait FromBytes<ViaProtocol>: Sized {
    fn from_bytes(bytes: &[u8]) -> Result<Self, ProtoConversionError>;
}

impl<P, N> FromBytes<P> for N
where
    P: protobuf::Message,
    N: FromProto<P>,
{
    fn from_bytes(bytes: &[u8]) -> Result<Self, ProtoConversionError> {
        let p: P = protobuf::parse_from_bytes(bytes)
            .map_err(|err| ProtoConversionError::DeserializationError(err.to_string()))?;
        N::from_proto(p)
    }
}

/// Convert to bytes from a native struct, via the given protocol.
pub trait IntoBytes<ViaProtocol>: Sized {
    fn into_bytes(self) -> Result<Vec<u8>, ProtoConversionError>;
}

impl<P, N> IntoBytes<P> for N
where
    P: protobuf::Message + FromNative<N>,
{
    fn into_bytes(self) -> Result<Vec<u8>, ProtoConversionError> {
        let p = P::from_native(self)?;
        p.write_to_bytes()
            .map_err(|err| ProtoConversionError::SerializationError(err.to_string()))
    }
}

pub trait IntoNative<T>: Sized
where
    T: FromProto<Self>,
{
    fn into_native(self) -> Result<T, ProtoConversionError> {
        FromProto::from_proto(self)
    }
}

pub trait IntoProto<T>: Sized
where
    T: FromNative<Self>,
{
    fn into_proto(self) -> Result<T, ProtoConversionError> {
        FromNative::from_native(self)
    }
}

include!(concat!(env!("OUT_DIR"), "/protos/mod.rs"));
