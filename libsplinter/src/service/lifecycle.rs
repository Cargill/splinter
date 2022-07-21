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

//! Contains `Lifecycle` trait.

use crate::error::InternalError;
use crate::store::command::StoreCommand;

use super::{ArgumentsConverter, FullyQualifiedServiceId};

/// Moves a service through its lifecycle.
///
/// When implementing `Lifecycle`, one generic is provided for the context type that will be used
/// buy the `StoreCommand`s. Every service type needs to implement this trait as it is used by
/// the `LifecycleExecutor` to update a service status.
pub trait Lifecycle<K> {
    type Arguments;

    /// Return a `StoreCommand` for adding a service that will be in the prepared state. The
    /// service after the command is run should be ready to handle incoming messages.
    /// Any associated timer operations should not yet be running.
    ///
    /// # Arguments
    ///
    /// * `service` - The fully qualified service ID for the service being added
    /// * `arguments` - The arguments for the service
    fn command_to_prepare(
        &self,
        service: FullyQualifiedServiceId,
        arguments: Self::Arguments,
    ) -> Result<Box<dyn StoreCommand<Context = K>>, InternalError>;

    /// Return a `StoreCommand` for updating a service to the finalized state. The
    /// service after the command is run should be ready to handle timer operations.
    ///
    /// # Arguments
    ///
    /// * `service` - The fully qualified service ID for the service being finalized
    fn command_to_finalize(
        &self,
        service: FullyQualifiedServiceId,
    ) -> Result<Box<dyn StoreCommand<Context = K>>, InternalError>;

    /// Return a `StoreCommand` for updating a service to the retired state. The
    /// service after the command is run should not longer handle messages.
    ///
    /// # Arguments
    ///
    /// * `service` - The fully qualified service ID for the service being retired
    fn command_to_retire(
        &self,
        service: FullyQualifiedServiceId,
    ) -> Result<Box<dyn StoreCommand<Context = K>>, InternalError>;

    /// Return a `StoreCommand` for purging a service. The service after the command is run should
    /// be completely remove from state.
    ///
    /// # Arguments
    ///
    /// * `service` - The fully qualified service ID for the service being purged
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
