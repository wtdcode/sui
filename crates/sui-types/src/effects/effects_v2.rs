// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::object_change::{AccumulatorWriteV1, ObjectIn, ObjectOut};
use super::{EffectsObjectChange, IDOperation, ObjectChange};
use crate::accumulator_event::AccumulatorEvent;
use crate::base_types::{
    EpochId, ObjectDigest, ObjectID, ObjectRef, SequenceNumber, SuiAddress, TransactionDigest,
    VersionDigest,
};
use crate::digests::{EffectsAuxDataDigest, TransactionEventsDigest};
use crate::effects::{InputSharedObject, TransactionEffectsAPI};
use crate::execution::SharedInput;
use crate::execution_status::{ExecutionFailureStatus, ExecutionStatus, MoveLocation};
use crate::gas::GasCostSummary;
#[cfg(debug_assertions)]
use crate::is_system_package;
use crate::object::{Owner, OBJECT_START_VERSION};
use serde::{Deserialize, Serialize};
#[cfg(debug_assertions)]
use std::collections::HashSet;
use std::collections::{BTreeMap, BTreeSet};

/// The response from processing a transaction or a certified transaction
#[derive(Eq, PartialEq, Clone, Debug, Serialize, Deserialize)]
pub struct TransactionEffectsV2 {
    /// The status of the execution
    pub(crate) status: ExecutionStatus,
    /// The epoch when this transaction was executed.
    pub(crate) executed_epoch: EpochId,
    pub(crate) gas_used: GasCostSummary,
    /// The transaction digest
    pub(crate) transaction_digest: TransactionDigest,
    /// The updated gas object reference, as an index into the `changed_objects` vector.
    /// Having a dedicated field for convenient access.
    /// System transaction that don't require gas will leave this as None.
    pub(crate) gas_object_index: Option<u32>,
    /// The digest of the events emitted during execution,
    /// can be None if the transaction does not emit any event.
    pub(crate) events_digest: Option<TransactionEventsDigest>,
    /// The set of transaction digests this transaction depends on.
    pub(crate) dependencies: Vec<TransactionDigest>,

    /// The version number of all the written Move objects by this transaction.
    pub(crate) lamport_version: SequenceNumber,
    /// Objects whose state are changed in the object store.
    /// This field should not be exposed to the public API.
    /// Otherwise it will make it harder to use effects of different versions.
    /// Note that for accumulator writes, the ObjectID here will be the dynamic field object ID
    /// that stores the accumulator value. However this object is not really mutated
    /// in this transaction. We just have to use an ObjectID that is unique so that
    /// it does not conflict with any other object IDs in the changed_objects.
    pub(crate) changed_objects: Vec<(ObjectID, EffectsObjectChange)>,
    /// Shared objects that are not mutated in this transaction. Unlike owned objects,
    /// read-only shared objects' version are not committed in the transaction,
    /// and in order for a node to catch up and execute it without consensus sequencing,
    /// the version needs to be committed in the effects.
    pub(crate) unchanged_shared_objects: Vec<(ObjectID, UnchangedSharedKind)>,
    /// Auxiliary data that are not protocol-critical, generated as part of the effects but are stored separately.
    /// Storing it separately allows us to avoid bloating the effects with data that are not critical.
    /// It also provides more flexibility on the format and type of the data.
    pub(crate) aux_data_digest: Option<EffectsAuxDataDigest>,
}

impl TransactionEffectsAPI for TransactionEffectsV2 {
    fn status(&self) -> &ExecutionStatus {
        &self.status
    }

    fn into_status(self) -> ExecutionStatus {
        self.status
    }

    fn executed_epoch(&self) -> EpochId {
        self.executed_epoch
    }

    fn modified_at_versions(&self) -> Vec<(ObjectID, SequenceNumber)> {
        self.changed_objects
            .iter()
            .filter_map(|(id, change)| {
                if let ObjectIn::Exist(((version, _digest), _owner)) = &change.input_state {
                    Some((*id, *version))
                } else {
                    None
                }
            })
            .collect()
    }

    fn move_abort(&self) -> Option<(MoveLocation, u64)> {
        let ExecutionStatus::Failure {
            error: ExecutionFailureStatus::MoveAbort(move_location, code),
            ..
        } = self.status()
        else {
            return None;
        };
        Some((move_location.clone(), *code))
    }

    fn lamport_version(&self) -> SequenceNumber {
        self.lamport_version
    }

    fn old_object_metadata(&self) -> Vec<(ObjectRef, Owner)> {
        self.changed_objects
            .iter()
            .filter_map(|(id, change)| {
                if let ObjectIn::Exist(((version, digest), owner)) = &change.input_state {
                    Some(((*id, *version, *digest), owner.clone()))
                } else {
                    None
                }
            })
            .collect()
    }

    fn input_shared_objects(&self) -> Vec<InputSharedObject> {
        self.changed_objects
            .iter()
            .filter_map(|(id, change)| match &change.input_state {
                ObjectIn::Exist(((version, digest), owner)) if owner.is_consensus() => {
                    Some(InputSharedObject::Mutate((*id, *version, *digest)))
                }
                _ => None,
            })
            .chain(
                self.unchanged_shared_objects
                    .iter()
                    .filter_map(|(id, change_kind)| match change_kind {
                        UnchangedSharedKind::ReadOnlyRoot((version, digest)) => {
                            Some(InputSharedObject::ReadOnly((*id, *version, *digest)))
                        }
                        UnchangedSharedKind::MutateConsensusStreamEnded(seqno) => {
                            Some(InputSharedObject::MutateConsensusStreamEnded(*id, *seqno))
                        }
                        UnchangedSharedKind::ReadConsensusStreamEnded(seqno) => {
                            Some(InputSharedObject::ReadConsensusStreamEnded(*id, *seqno))
                        }
                        UnchangedSharedKind::Cancelled(seqno) => {
                            Some(InputSharedObject::Cancelled(*id, *seqno))
                        }
                        // We can not expose the per epoch config object as input shared object,
                        // since it does not require sequencing, and hence shall not be considered
                        // as a normal input shared object.
                        UnchangedSharedKind::PerEpochConfig => None,
                    }),
            )
            .collect()
    }

    fn created(&self) -> Vec<(ObjectRef, Owner)> {
        self.changed_objects
            .iter()
            .filter_map(|(id, change)| {
                match (
                    &change.input_state,
                    &change.output_state,
                    &change.id_operation,
                ) {
                    (
                        ObjectIn::NotExist,
                        ObjectOut::ObjectWrite((digest, owner)),
                        IDOperation::Created,
                    ) => Some(((*id, self.lamport_version, *digest), owner.clone())),
                    (
                        ObjectIn::NotExist,
                        ObjectOut::PackageWrite((version, digest)),
                        IDOperation::Created,
                    ) => Some(((*id, *version, *digest), Owner::Immutable)),
                    _ => None,
                }
            })
            .collect()
    }

    fn mutated(&self) -> Vec<(ObjectRef, Owner)> {
        self.changed_objects
            .iter()
            .filter_map(
                |(id, change)| match (&change.input_state, &change.output_state) {
                    (ObjectIn::Exist(_), ObjectOut::ObjectWrite((digest, owner))) => {
                        Some(((*id, self.lamport_version, *digest), owner.clone()))
                    }
                    (ObjectIn::Exist(_), ObjectOut::PackageWrite((version, digest))) => {
                        Some(((*id, *version, *digest), Owner::Immutable))
                    }
                    _ => None,
                },
            )
            .collect()
    }

    fn unwrapped(&self) -> Vec<(ObjectRef, Owner)> {
        self.changed_objects
            .iter()
            .filter_map(|(id, change)| {
                match (
                    &change.input_state,
                    &change.output_state,
                    &change.id_operation,
                ) {
                    (
                        ObjectIn::NotExist,
                        ObjectOut::ObjectWrite((digest, owner)),
                        IDOperation::None,
                    ) => Some(((*id, self.lamport_version, *digest), owner.clone())),
                    _ => None,
                }
            })
            .collect()
    }

    fn deleted(&self) -> Vec<ObjectRef> {
        self.changed_objects
            .iter()
            .filter_map(|(id, change)| {
                match (
                    &change.input_state,
                    &change.output_state,
                    &change.id_operation,
                ) {
                    (ObjectIn::Exist(_), ObjectOut::NotExist, IDOperation::Deleted) => Some((
                        *id,
                        self.lamport_version,
                        ObjectDigest::OBJECT_DIGEST_DELETED,
                    )),
                    _ => None,
                }
            })
            .collect()
    }

    fn unwrapped_then_deleted(&self) -> Vec<ObjectRef> {
        self.changed_objects
            .iter()
            .filter_map(|(id, change)| {
                match (
                    &change.input_state,
                    &change.output_state,
                    &change.id_operation,
                ) {
                    (ObjectIn::NotExist, ObjectOut::NotExist, IDOperation::Deleted) => Some((
                        *id,
                        self.lamport_version,
                        ObjectDigest::OBJECT_DIGEST_DELETED,
                    )),
                    _ => None,
                }
            })
            .collect()
    }

    fn wrapped(&self) -> Vec<ObjectRef> {
        self.changed_objects
            .iter()
            .filter_map(|(id, change)| {
                match (
                    &change.input_state,
                    &change.output_state,
                    &change.id_operation,
                ) {
                    (ObjectIn::Exist(_), ObjectOut::NotExist, IDOperation::None) => Some((
                        *id,
                        self.lamport_version,
                        ObjectDigest::OBJECT_DIGEST_WRAPPED,
                    )),
                    _ => None,
                }
            })
            .collect()
    }

    fn transferred_from_consensus(&self) -> Vec<ObjectRef> {
        self.changed_objects
            .iter()
            .filter_map(|(id, change)| {
                match (
                    &change.input_state,
                    &change.output_state,
                    &change.id_operation,
                ) {
                    (
                        ObjectIn::Exist((_, Owner::ConsensusAddressOwner { .. })),
                        ObjectOut::ObjectWrite((
                            object_digest,
                            Owner::AddressOwner(_) | Owner::ObjectOwner(_) | Owner::Immutable,
                        )),
                        IDOperation::None,
                    ) => Some((*id, self.lamport_version, *object_digest)),
                    _ => None,
                }
            })
            .collect()
    }

    fn transferred_to_consensus(&self) -> Vec<ObjectRef> {
        self.changed_objects
            .iter()
            .filter_map(|(id, change)| {
                match (
                    &change.input_state,
                    &change.output_state,
                    &change.id_operation,
                ) {
                    (
                        ObjectIn::Exist((_, Owner::AddressOwner(_) | Owner::ObjectOwner(_))),
                        ObjectOut::ObjectWrite((
                            object_digest,
                            Owner::ConsensusAddressOwner { .. },
                        )),
                        IDOperation::None,
                    ) => Some((*id, self.lamport_version, *object_digest)),
                    _ => None,
                }
            })
            .collect()
    }

    fn consensus_owner_changed(&self) -> Vec<ObjectRef> {
        self.changed_objects
            .iter()
            .filter_map(|(id, change)| {
                match (
                    &change.input_state,
                    &change.output_state,
                    &change.id_operation,
                ) {
                    (
                        ObjectIn::Exist((
                            _,
                            Owner::ConsensusAddressOwner {
                                owner: old_owner, ..
                            },
                        )),
                        ObjectOut::ObjectWrite((
                            object_digest,
                            Owner::ConsensusAddressOwner {
                                owner: new_owner, ..
                            },
                        )),
                        IDOperation::None,
                    ) if old_owner != new_owner => {
                        Some((*id, self.lamport_version, *object_digest))
                    }
                    _ => None,
                }
            })
            .collect()
    }

    fn object_changes(&self) -> Vec<ObjectChange> {
        self.changed_objects
            .iter()
            .filter_map(|(id, change)| {
                let input_version_digest = match &change.input_state {
                    ObjectIn::NotExist => None,
                    ObjectIn::Exist((vd, _)) => Some(*vd),
                };

                let output_version_digest = match &change.output_state {
                    ObjectOut::NotExist => None,
                    ObjectOut::ObjectWrite((d, _)) => Some((self.lamport_version, *d)),
                    ObjectOut::PackageWrite(vd) => Some(*vd),
                    ObjectOut::AccumulatorWriteV1(_) => {
                        return None;
                    }
                };

                Some(ObjectChange {
                    id: *id,

                    input_version: input_version_digest.map(|k| k.0),
                    input_digest: input_version_digest.map(|k| k.1),

                    output_version: output_version_digest.map(|k| k.0),
                    output_digest: output_version_digest.map(|k| k.1),

                    id_operation: change.id_operation,
                })
            })
            .collect()
    }

    fn accumulator_events(&self) -> Vec<AccumulatorEvent> {
        self.changed_objects
            .iter()
            .filter_map(|(id, change)| match &change.output_state {
                ObjectOut::AccumulatorWriteV1(write) => {
                    Some(AccumulatorEvent::new(*id, write.clone()))
                }
                _ => None,
            })
            .collect()
    }

    fn gas_object(&self) -> (ObjectRef, Owner) {
        if let Some(gas_object_index) = self.gas_object_index {
            let entry = &self.changed_objects[gas_object_index as usize];
            match &entry.1.output_state {
                ObjectOut::ObjectWrite((digest, owner)) => {
                    ((entry.0, self.lamport_version, *digest), owner.clone())
                }
                _ => panic!("Gas object must be an ObjectWrite in changed_objects"),
            }
        } else {
            (
                (ObjectID::ZERO, SequenceNumber::default(), ObjectDigest::MIN),
                Owner::AddressOwner(SuiAddress::default()),
            )
        }
    }

    fn events_digest(&self) -> Option<&TransactionEventsDigest> {
        self.events_digest.as_ref()
    }

    fn dependencies(&self) -> &[TransactionDigest] {
        &self.dependencies
    }

    fn transaction_digest(&self) -> &TransactionDigest {
        &self.transaction_digest
    }

    fn gas_cost_summary(&self) -> &GasCostSummary {
        &self.gas_used
    }

    fn unchanged_shared_objects(&self) -> Vec<(ObjectID, UnchangedSharedKind)> {
        self.unchanged_shared_objects.clone()
    }

    fn accumulator_updates(&self) -> Vec<(ObjectID, AccumulatorWriteV1)> {
        self.changed_objects
            .iter()
            .filter_map(|(id, change)| match &change.output_state {
                ObjectOut::AccumulatorWriteV1(update) => Some((*id, update.clone())),
                _ => None,
            })
            .collect()
    }

    fn status_mut_for_testing(&mut self) -> &mut ExecutionStatus {
        &mut self.status
    }

    fn gas_cost_summary_mut_for_testing(&mut self) -> &mut GasCostSummary {
        &mut self.gas_used
    }

    fn transaction_digest_mut_for_testing(&mut self) -> &mut TransactionDigest {
        &mut self.transaction_digest
    }

    fn dependencies_mut_for_testing(&mut self) -> &mut Vec<TransactionDigest> {
        &mut self.dependencies
    }

    fn unsafe_add_input_shared_object_for_testing(&mut self, kind: InputSharedObject) {
        match kind {
            InputSharedObject::Mutate(obj_ref) => self.changed_objects.push((
                obj_ref.0,
                EffectsObjectChange {
                    input_state: ObjectIn::Exist((
                        (obj_ref.1, obj_ref.2),
                        Owner::Shared {
                            initial_shared_version: OBJECT_START_VERSION,
                        },
                    )),
                    output_state: ObjectOut::ObjectWrite((
                        obj_ref.2,
                        Owner::Shared {
                            initial_shared_version: obj_ref.1,
                        },
                    )),
                    id_operation: IDOperation::None,
                },
            )),
            InputSharedObject::ReadOnly(obj_ref) => self.unchanged_shared_objects.push((
                obj_ref.0,
                UnchangedSharedKind::ReadOnlyRoot((obj_ref.1, obj_ref.2)),
            )),
            InputSharedObject::ReadConsensusStreamEnded(obj_id, seqno) => self
                .unchanged_shared_objects
                .push((obj_id, UnchangedSharedKind::ReadConsensusStreamEnded(seqno))),
            InputSharedObject::MutateConsensusStreamEnded(obj_id, seqno) => {
                self.unchanged_shared_objects.push((
                    obj_id,
                    UnchangedSharedKind::MutateConsensusStreamEnded(seqno),
                ))
            }
            InputSharedObject::Cancelled(obj_id, seqno) => self
                .unchanged_shared_objects
                .push((obj_id, UnchangedSharedKind::Cancelled(seqno))),
        }
    }

    fn unsafe_add_deleted_live_object_for_testing(&mut self, obj_ref: ObjectRef) {
        self.changed_objects.push((
            obj_ref.0,
            EffectsObjectChange {
                input_state: ObjectIn::Exist((
                    (obj_ref.1, obj_ref.2),
                    Owner::AddressOwner(SuiAddress::default()),
                )),
                output_state: ObjectOut::ObjectWrite((
                    obj_ref.2,
                    Owner::AddressOwner(SuiAddress::default()),
                )),
                id_operation: IDOperation::None,
            },
        ))
    }

    fn unsafe_add_object_tombstone_for_testing(&mut self, obj_ref: ObjectRef) {
        self.changed_objects.push((
            obj_ref.0,
            EffectsObjectChange {
                input_state: ObjectIn::Exist((
                    (obj_ref.1, obj_ref.2),
                    Owner::AddressOwner(SuiAddress::default()),
                )),
                output_state: ObjectOut::NotExist,
                id_operation: IDOperation::Deleted,
            },
        ))
    }
}

impl TransactionEffectsV2 {
    pub fn new(
        status: ExecutionStatus,
        executed_epoch: EpochId,
        gas_used: GasCostSummary,
        shared_objects: Vec<SharedInput>,
        loaded_per_epoch_config_objects: BTreeSet<ObjectID>,
        transaction_digest: TransactionDigest,
        lamport_version: SequenceNumber,
        changed_objects: BTreeMap<ObjectID, EffectsObjectChange>,
        gas_object: Option<ObjectID>,
        events_digest: Option<TransactionEventsDigest>,
        dependencies: Vec<TransactionDigest>,
    ) -> Self {
        let unchanged_shared_objects = shared_objects
            .into_iter()
            .filter_map(|shared_input| match shared_input {
                SharedInput::Existing((id, version, digest)) => {
                    if changed_objects.contains_key(&id) {
                        None
                    } else {
                        Some((id, UnchangedSharedKind::ReadOnlyRoot((version, digest))))
                    }
                }
                SharedInput::ConsensusStreamEnded((id, version, mutable, _)) => {
                    debug_assert!(!changed_objects.contains_key(&id));
                    if mutable {
                        Some((id, UnchangedSharedKind::MutateConsensusStreamEnded(version)))
                    } else {
                        Some((id, UnchangedSharedKind::ReadConsensusStreamEnded(version)))
                    }
                }
                SharedInput::Cancelled((id, version)) => {
                    debug_assert!(!changed_objects.contains_key(&id));
                    Some((id, UnchangedSharedKind::Cancelled(version)))
                }
            })
            .chain(
                loaded_per_epoch_config_objects
                    .into_iter()
                    .map(|id| (id, UnchangedSharedKind::PerEpochConfig)),
            )
            .collect();
        let changed_objects: Vec<_> = changed_objects.into_iter().collect();

        let gas_object_index = gas_object.map(|gas_id| {
            changed_objects
                .iter()
                .position(|(id, _)| id == &gas_id)
                .unwrap() as u32
        });

        let result = Self {
            status,
            executed_epoch,
            gas_used,
            transaction_digest,
            lamport_version,
            changed_objects,
            unchanged_shared_objects,
            gas_object_index,
            events_digest,
            dependencies,
            aux_data_digest: None,
        };
        #[cfg(debug_assertions)]
        result.check_invariant();

        result
    }

    /// This function demonstrates what's the invariant of the effects.
    /// It also documents the semantics of different combinations in object changes.
    #[cfg(debug_assertions)]
    fn check_invariant(&self) {
        let mut unique_ids = HashSet::new();
        for (id, change) in &self.changed_objects {
            assert!(unique_ids.insert(*id));
            match (
                &change.input_state,
                &change.output_state,
                &change.id_operation,
            ) {
                (ObjectIn::NotExist, ObjectOut::NotExist, IDOperation::Created) => {
                    // created and then wrapped Move object.
                }
                (ObjectIn::NotExist, ObjectOut::NotExist, IDOperation::Deleted) => {
                    // unwrapped and then deleted Move object.
                }
                (ObjectIn::NotExist, ObjectOut::ObjectWrite((_, owner)), IDOperation::None) => {
                    // unwrapped Move object.
                    // It's not allowed to make an object shared after unwrapping.
                    assert!(!owner.is_shared());
                }
                (ObjectIn::NotExist, ObjectOut::ObjectWrite(..), IDOperation::Created) => {
                    // created Move object.
                }
                (ObjectIn::NotExist, ObjectOut::PackageWrite(_), IDOperation::Created) => {
                    // created Move package or user Move package upgrade.
                }
                (
                    ObjectIn::Exist(((old_version, _), old_owner)),
                    ObjectOut::NotExist,
                    IDOperation::None,
                ) => {
                    // wrapped.
                    assert!(old_version.value() < self.lamport_version.value());
                    assert!(
                        !old_owner.is_shared() && !old_owner.is_immutable(),
                        "Cannot wrap shared or immutable object"
                    );
                }
                (
                    ObjectIn::Exist(((old_version, _), old_owner)),
                    ObjectOut::NotExist,
                    IDOperation::Deleted,
                ) => {
                    // deleted.
                    assert!(old_version.value() < self.lamport_version.value());
                    assert!(!old_owner.is_immutable(), "Cannot delete immutable object");
                }
                (
                    ObjectIn::Exist(((old_version, old_digest), old_owner)),
                    ObjectOut::ObjectWrite((new_digest, new_owner)),
                    IDOperation::None,
                ) => {
                    // mutated.
                    assert!(old_version.value() < self.lamport_version.value());
                    assert_ne!(old_digest, new_digest);
                    assert!(!old_owner.is_immutable(), "Cannot mutate immutable object");
                    if old_owner.is_shared() {
                        assert!(new_owner.is_shared(), "Cannot un-share an object");
                    } else {
                        assert!(!new_owner.is_shared(), "Cannot share an existing object");
                    }
                }
                (
                    ObjectIn::Exist(((old_version, old_digest), old_owner)),
                    ObjectOut::PackageWrite((new_version, new_digest)),
                    IDOperation::None,
                ) => {
                    // system package upgrade.
                    assert!(
                        old_owner.is_immutable() && is_system_package(*id),
                        "Must be a system package"
                    );
                    assert_eq!(old_version.value() + 1, new_version.value());
                    assert_ne!(old_digest, new_digest);
                }
                (ObjectIn::NotExist, ObjectOut::AccumulatorWriteV1(_), IDOperation::None) => {
                    // This is an accumulator write.
                }
                _ => {
                    panic!("Impossible object change: {:?}, {:?}", id, change);
                }
            }
        }
        // Make sure that gas object exists in changed_objects.
        let (_, owner) = self.gas_object();
        assert!(matches!(owner, Owner::AddressOwner(_)));

        for (id, _) in &self.unchanged_shared_objects {
            assert!(
                unique_ids.insert(*id),
                "Duplicate object id: {:?}\n{:#?}",
                id,
                self
            );
        }
    }
}

impl Default for TransactionEffectsV2 {
    fn default() -> Self {
        Self {
            status: ExecutionStatus::Success,
            executed_epoch: 0,
            gas_used: GasCostSummary::default(),
            transaction_digest: TransactionDigest::default(),
            lamport_version: SequenceNumber::default(),
            changed_objects: vec![],
            unchanged_shared_objects: vec![],
            gas_object_index: None,
            events_digest: None,
            dependencies: vec![],
            aux_data_digest: None,
        }
    }
}

#[derive(Eq, PartialEq, Clone, Debug, Serialize, Deserialize)]
pub enum UnchangedSharedKind {
    /// Read-only shared objects from the input. We don't really need ObjectDigest
    /// for protocol correctness, but it will make it easier to verify untrusted read.
    ReadOnlyRoot(VersionDigest),
    /// Objects with ended consensus streams that appear mutably/owned in the input.
    MutateConsensusStreamEnded(SequenceNumber),
    /// Objects with ended consensus streams objects that appear as read-only in the input.
    ReadConsensusStreamEnded(SequenceNumber),
    /// Shared objects in cancelled transaction. The sequence number embed cancellation reason.
    Cancelled(SequenceNumber),
    /// Read of a per-epoch config object that should remain the same during an epoch.
    PerEpochConfig,
}
