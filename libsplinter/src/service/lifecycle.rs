// Copyright 2018-2022 Cargill Incorporated
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

use crate::error::InternalError;
use crate::store::command::StoreCommand;

use super::{ArgumentsConverter, FullyQualifiedServiceId};

pub trait Lifecycle<K> {
    type Arguments;

    fn command_to_prepare(
        &self,
        service: FullyQualifiedServiceId,
        arguments: Self::Arguments,
    ) -> Result<Box<dyn StoreCommand<Context = K>>, InternalError>;

    fn command_to_finalize(
        &self,
        service: FullyQualifiedServiceId,
    ) -> Result<Box<dyn StoreCommand<Context = K>>, InternalError>;

    fn command_to_retire(
        &self,
        service: FullyQualifiedServiceId,
    ) -> Result<Box<dyn StoreCommand<Context = K>>, InternalError>;

    fn command_to_purge(
        &self,
        service: FullyQualifiedServiceId,
    ) -> Result<Box<dyn StoreCommand<Context = K>>, InternalError>;

    fn into_lifecycle<C, R>(self, converter: C) -> IntoLifecycle<Self, C, Self::Arguments, R, K>
    where
        Self: Sized,
        C: ArgumentsConverter<Self::Arguments, R>,
    {
        IntoLifecycle::new(self, converter)
    }
}

pub struct IntoLifecycle<I, C, L, R, K> {
    inner: I,
    converter: C,
    _left: std::marker::PhantomData<L>,
    _right: std::marker::PhantomData<R>,
    _k: std::marker::PhantomData<K>,
}

impl<I, C, L, R, K> IntoLifecycle<I, C, L, R, K>
where
    I: Lifecycle<K, Arguments = L>,
    C: ArgumentsConverter<L, R>,
{
    fn new(inner: I, converter: C) -> Self {
        Self {
            inner,
            converter,
            _left: std::marker::PhantomData,
            _right: std::marker::PhantomData,
            _k: std::marker::PhantomData,
        }
    }
}

impl<I, C, L, R, K> Lifecycle<K> for IntoLifecycle<I, C, L, R, K>
where
    I: Lifecycle<K, Arguments = L>,
    C: ArgumentsConverter<L, R>,
{
    type Arguments = R;

    fn command_to_prepare(
        &self,
        service: FullyQualifiedServiceId,
        arguments: Self::Arguments,
    ) -> Result<Box<dyn StoreCommand<Context = K>>, InternalError> {
        let left_arguments = self.converter.to_left(arguments)?;
        self.inner.command_to_prepare(service, left_arguments)
    }

    fn command_to_finalize(
        &self,
        service: FullyQualifiedServiceId,
    ) -> Result<Box<dyn StoreCommand<Context = K>>, InternalError> {
        self.inner.command_to_finalize(service)
    }

    fn command_to_retire(
        &self,
        service: FullyQualifiedServiceId,
    ) -> Result<Box<dyn StoreCommand<Context = K>>, InternalError> {
        self.inner.command_to_retire(service)
    }

    fn command_to_purge(
        &self,
        service: FullyQualifiedServiceId,
    ) -> Result<Box<dyn StoreCommand<Context = K>>, InternalError> {
        self.inner.command_to_purge(service)
    }
}
