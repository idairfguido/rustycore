use std::collections::{HashMap, HashSet};

use wow_constants::SpellState;
use wow_core::ObjectGuid;

/// Minimal bridge for TrinityCore `Unit` aura containers.
///
/// This is metadata/state only: it does not run aura scripts, periodic ticks, proc logic,
/// packet emission, or update-field masking by itself.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct AuraSubsystem {
    pub owned_auras: Vec<OwnedAuraRef>,
    pub applied_auras: Vec<AppliedAuraRef>,
    pub visible_auras: HashMap<u8, AuraRef>,
    pub removed_auras: Vec<AuraRef>,
    pub interrupt_flags: u32,
    pub interrupt_flags2: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AuraRef {
    pub spell_id: u32,
    pub caster_guid: ObjectGuid,
}

impl AuraRef {
    pub const fn new(spell_id: u32, caster_guid: ObjectGuid) -> Self {
        Self {
            spell_id,
            caster_guid,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct OwnedAuraRef {
    pub spell_id: u32,
    pub caster_guid: ObjectGuid,
    pub item_caster_guid: Option<ObjectGuid>,
}

impl OwnedAuraRef {
    pub const fn new(
        spell_id: u32,
        caster_guid: ObjectGuid,
        item_caster_guid: Option<ObjectGuid>,
    ) -> Self {
        Self {
            spell_id,
            caster_guid,
            item_caster_guid,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AppliedAuraRef {
    pub spell_id: u32,
    pub caster_guid: ObjectGuid,
    pub slot: u8,
    pub effect_mask: u32,
}

impl AppliedAuraRef {
    pub const fn new(spell_id: u32, caster_guid: ObjectGuid, slot: u8, effect_mask: u32) -> Self {
        Self {
            spell_id,
            caster_guid,
            slot,
            effect_mask,
        }
    }
}

impl AuraSubsystem {
    pub fn add_owned(&mut self, aura: OwnedAuraRef) {
        if !self.owned_auras.contains(&aura) {
            self.owned_auras.push(aura);
        }
    }

    pub fn remove_owned(&mut self, aura: OwnedAuraRef) -> bool {
        let before = self.owned_auras.len();
        self.owned_auras.retain(|known| *known != aura);
        before != self.owned_auras.len()
    }

    pub fn has_owned(&self, aura: OwnedAuraRef) -> bool {
        self.owned_auras.contains(&aura)
    }

    pub fn add_applied(&mut self, aura: AppliedAuraRef) {
        if !self.applied_auras.contains(&aura) {
            self.applied_auras.push(aura);
        }
    }

    pub fn remove_applied(&mut self, aura: AppliedAuraRef) -> bool {
        let before = self.applied_auras.len();
        self.applied_auras.retain(|known| *known != aura);
        before != self.applied_auras.len()
    }

    pub fn has_applied(&self, aura: AppliedAuraRef) -> bool {
        self.applied_auras.contains(&aura)
    }

    pub fn set_visible(&mut self, slot: u8, aura: AuraRef) {
        self.visible_auras.insert(slot, aura);
    }

    pub fn clear_visible(&mut self, slot: u8) -> Option<AuraRef> {
        self.visible_auras.remove(&slot)
    }

    pub fn mark_removed(&mut self, aura: AuraRef) {
        self.removed_auras.push(aura);
    }

    pub fn clear_removed(&mut self) {
        self.removed_auras.clear();
    }

    pub fn removed_count(&self) -> usize {
        self.removed_auras.len()
    }
}

/// Trinity-compatible current spell slots represented in RustyCore state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum CurrentSpellSlot {
    Melee = 0,
    Generic = 1,
    Channeled = 2,
    Autorepeat = 3,
}

pub const CURRENT_FIRST_NON_MELEE_SPELL: u8 = 1;
pub const CURRENT_MAX_SPELL: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CurrentSpellRef {
    pub spell_id: u32,
    pub caster_guid: Option<ObjectGuid>,
    pub cast_id: Option<ObjectGuid>,
    pub cast_time_ms: u32,
    pub state: SpellState,
    pub interruptible: bool,
    pub allow_actions_during_channel: bool,
}

impl CurrentSpellRef {
    pub const fn new(
        spell_id: u32,
        caster_guid: Option<ObjectGuid>,
        cast_id: Option<ObjectGuid>,
    ) -> Self {
        Self {
            spell_id,
            caster_guid,
            cast_id,
            cast_time_ms: 0,
            state: SpellState::None,
            interruptible: true,
            allow_actions_during_channel: false,
        }
    }

    pub const fn with_cast_time_ms(mut self, cast_time_ms: u32) -> Self {
        self.cast_time_ms = cast_time_ms;
        self
    }

    pub const fn with_state(mut self, state: SpellState) -> Self {
        self.state = state;
        self
    }

    pub const fn with_interruptible(mut self, interruptible: bool) -> Self {
        self.interruptible = interruptible;
        self
    }

    pub const fn with_allow_actions_during_channel(
        mut self,
        allow_actions_during_channel: bool,
    ) -> Self {
        self.allow_actions_during_channel = allow_actions_during_channel;
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpellCooldown {
    pub started_at_ms: u64,
    pub duration_ms: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpellChargeState {
    pub charges: u8,
    pub started_at_ms: u64,
    pub recharge_ms: u32,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct SpellHistory {
    pub cooldowns: HashMap<u32, SpellCooldown>,
    pub charges: HashMap<u32, SpellChargeState>,
}

impl SpellHistory {
    pub fn set_cooldown(&mut self, spell_id: u32, started_at_ms: u64, duration_ms: u32) {
        self.cooldowns.insert(
            spell_id,
            SpellCooldown {
                started_at_ms,
                duration_ms,
            },
        );
    }

    pub fn cooldown(&self, spell_id: u32) -> Option<SpellCooldown> {
        self.cooldowns.get(&spell_id).copied()
    }

    pub fn clear_cooldown(&mut self, spell_id: u32) -> bool {
        self.cooldowns.remove(&spell_id).is_some()
    }

    pub fn set_charges(
        &mut self,
        spell_id: u32,
        charges: u8,
        started_at_ms: u64,
        recharge_ms: u32,
    ) {
        self.charges.insert(
            spell_id,
            SpellChargeState {
                charges,
                started_at_ms,
                recharge_ms,
            },
        );
    }

    pub fn charges(&self, spell_id: u32) -> Option<SpellChargeState> {
        self.charges.get(&spell_id).copied()
    }

    pub fn clear_charges(&mut self, spell_id: u32) -> bool {
        self.charges.remove(&spell_id).is_some()
    }

    pub fn reset(&mut self) {
        self.cooldowns.clear();
        self.charges.clear();
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct SpellSubsystem {
    pub current_spells: HashMap<CurrentSpellSlot, CurrentSpellRef>,
    pub history: SpellHistory,
}

impl SpellSubsystem {
    pub fn set_current_spell(&mut self, slot: CurrentSpellSlot, spell: CurrentSpellRef) {
        self.current_spells.insert(slot, spell);
    }

    pub fn current_spell(&self, slot: CurrentSpellSlot) -> Option<CurrentSpellRef> {
        self.current_spells.get(&slot).copied()
    }

    pub fn clear_current_spell(&mut self, slot: CurrentSpellSlot) -> Option<CurrentSpellRef> {
        self.current_spells.remove(&slot)
    }

    pub fn clear_current_spells(&mut self) {
        self.current_spells.clear();
    }

    pub fn find_current_spell_by_spell_id(&self, spell_id: u32) -> Option<CurrentSpellRef> {
        self.current_spells
            .values()
            .find(|spell| spell.spell_id == spell_id)
            .copied()
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct CombatSubsystem {
    pub threat: HashMap<ObjectGuid, f32>,
    pub attackers: HashSet<ObjectGuid>,
    pub attacking_guid: Option<ObjectGuid>,
    pub combat_disallowed: bool,
}

impl CombatSubsystem {
    pub fn add_threat(&mut self, target: ObjectGuid, amount: f32) -> f32 {
        let value = self.threat.entry(target).or_insert(0.0);
        *value += amount;
        *value
    }

    pub fn set_threat(&mut self, target: ObjectGuid, value: f32) {
        self.threat.insert(target, value);
    }

    pub fn threat_value(&self, target: ObjectGuid) -> Option<f32> {
        self.threat.get(&target).copied()
    }

    pub fn remove_threat(&mut self, target: ObjectGuid) -> Option<f32> {
        self.threat.remove(&target)
    }

    pub fn clear_threat(&mut self) {
        self.threat.clear();
    }

    pub fn is_threatened_by(&self, target: ObjectGuid) -> bool {
        self.threat.contains_key(&target)
    }

    pub fn add_attacker(&mut self, attacker: ObjectGuid) -> bool {
        self.attackers.insert(attacker)
    }

    pub fn remove_attacker(&mut self, attacker: ObjectGuid) -> bool {
        self.attackers.remove(&attacker)
    }

    pub fn clear_attackers(&mut self) {
        self.attackers.clear();
        self.attacking_guid = None;
    }

    pub fn set_attacking(&mut self, victim: Option<ObjectGuid>) {
        self.attacking_guid = victim;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MovementGeneratorKind {
    Idle,
    Random,
    Waypoint,
    Chase,
    Follow,
    Fleeing,
    Confused,
    Custom(u32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MoveSplineState {
    pub enabled: bool,
    pub spline_id: u32,
    pub progress_ms: u32,
    pub duration_ms: u32,
}

impl Default for MoveSplineState {
    fn default() -> Self {
        Self {
            enabled: false,
            spline_id: 0,
            progress_ms: 0,
            duration_ms: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MotionSubsystem {
    pub default_generator: MovementGeneratorKind,
    pub current_generator: MovementGeneratorKind,
    pub paused: bool,
    pub stopped: bool,
    pub spline: MoveSplineState,
}

impl Default for MotionSubsystem {
    fn default() -> Self {
        Self {
            default_generator: MovementGeneratorKind::Idle,
            current_generator: MovementGeneratorKind::Idle,
            paused: false,
            stopped: false,
            spline: MoveSplineState::default(),
        }
    }
}

impl MotionSubsystem {
    pub fn set_current_generator(&mut self, generator: MovementGeneratorKind) {
        self.current_generator = generator;
        self.stopped = false;
    }

    pub fn pause_movement(&mut self) {
        self.paused = true;
    }

    pub fn resume_movement(&mut self) {
        self.paused = false;
    }

    pub fn stop_moving(&mut self) {
        self.stopped = true;
        self.spline.enabled = false;
        self.spline.progress_ms = 0;
    }

    pub fn start_spline(&mut self, spline_id: u32, duration_ms: u32) {
        self.spline = MoveSplineState {
            enabled: true,
            spline_id,
            progress_ms: 0,
            duration_ms,
        };
        self.stopped = false;
    }

    pub fn set_spline_progress(&mut self, progress_ms: u32) {
        self.spline.progress_ms = progress_ms.min(self.spline.duration_ms);
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct ControlSubsystem {
    pub last_charmer_guid: Option<ObjectGuid>,
    pub charmer_guid: Option<ObjectGuid>,
    pub charmed_guid: Option<ObjectGuid>,
    pub controlled_guids: HashSet<ObjectGuid>,
    pub controlled_by_player: bool,
    pub unit_moved_by_me: Option<ObjectGuid>,
    pub player_moving_me: Option<ObjectGuid>,
    pub shared_vision_guids: HashSet<ObjectGuid>,
    pub has_charm_info: bool,
}

impl ControlSubsystem {
    pub fn set_charmer(&mut self, charmer: ObjectGuid, controlled_by_player: bool) {
        self.last_charmer_guid = self.charmer_guid;
        self.charmer_guid = Some(charmer);
        self.controlled_by_player = controlled_by_player;
        self.has_charm_info = true;
    }

    pub fn remove_charmer(&mut self) {
        self.last_charmer_guid = self.charmer_guid;
        self.charmer_guid = None;
        self.controlled_by_player = false;
    }

    pub fn set_charmed(&mut self, charmed: ObjectGuid) {
        self.charmed_guid = Some(charmed);
        self.controlled_guids.insert(charmed);
        self.has_charm_info = true;
    }

    pub fn remove_charmed(&mut self) {
        if let Some(charmed) = self.charmed_guid.take() {
            self.controlled_guids.remove(&charmed);
        }
    }

    pub fn add_controlled(&mut self, guid: ObjectGuid) -> bool {
        self.controlled_guids.insert(guid)
    }

    pub fn remove_controlled(&mut self, guid: ObjectGuid) -> bool {
        if self.charmed_guid == Some(guid) {
            self.charmed_guid = None;
        }
        self.controlled_guids.remove(&guid)
    }

    pub fn clear_controlled(&mut self) {
        self.controlled_guids.clear();
        self.charmed_guid = None;
    }

    pub fn is_charmed(&self) -> bool {
        self.charmer_guid.is_some()
    }

    pub fn add_shared_vision(&mut self, guid: ObjectGuid) -> bool {
        self.shared_vision_guids.insert(guid)
    }

    pub fn remove_shared_vision(&mut self, guid: ObjectGuid) -> bool {
        self.shared_vision_guids.remove(&guid)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VehicleKitState {
    pub kit_id: u32,
    pub active: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct VehicleSubsystem {
    pub vehicle_guid: Option<ObjectGuid>,
    pub base_vehicle_guid: Option<ObjectGuid>,
    pub seat_id: Option<i8>,
    pub kit: Option<VehicleKitState>,
}

impl VehicleSubsystem {
    pub fn enter_vehicle(&mut self, vehicle_guid: ObjectGuid, seat_id: Option<i8>) {
        self.vehicle_guid = Some(vehicle_guid);
        self.seat_id = seat_id;
    }

    pub fn exit_vehicle(&mut self) {
        self.vehicle_guid = None;
        self.seat_id = None;
    }

    pub fn set_vehicle_kit(&mut self, kit_id: u32, active: bool) {
        self.kit = Some(VehicleKitState { kit_id, active });
    }

    pub fn clear_vehicle_kit(&mut self) {
        self.kit = None;
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AiSubsystem {
    pub active_ai: Option<String>,
    pub ai_stack: Vec<String>,
    pub locked: bool,
}

impl AiSubsystem {
    pub fn set_active(&mut self, ai: Option<impl Into<String>>) {
        self.active_ai = ai.map(Into::into);
    }

    pub fn push(&mut self, ai: impl Into<String>) {
        if let Some(active) = self.active_ai.take() {
            self.ai_stack.push(active);
        }
        self.active_ai = Some(ai.into());
    }

    pub fn pop(&mut self) -> Option<String> {
        let popped = self.active_ai.take();
        self.active_ai = self.ai_stack.pop();
        popped
    }

    pub fn set_locked(&mut self, locked: bool) {
        self.locked = locked;
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct UnitSubsystems {
    pub auras: AuraSubsystem,
    pub spells: SpellSubsystem,
    pub combat: CombatSubsystem,
    pub motion: MotionSubsystem,
    pub control: ControlSubsystem,
    pub vehicle: VehicleSubsystem,
    pub ai: AiSubsystem,
}

impl UnitSubsystems {
    pub fn clear_runtime_state(&mut self) {
        *self = Self::default();
    }
}

#[cfg(test)]
mod unit_subsystems_tests {
    use super::*;

    fn guid(low: i64) -> ObjectGuid {
        ObjectGuid::new(0, low)
    }

    #[test]
    fn aura_spell_history_and_current_spell_helpers_roundtrip() {
        let mut subsystems = UnitSubsystems::default();
        let caster = guid(1);
        let owned = OwnedAuraRef::new(100, caster, None);
        let applied = AppliedAuraRef::new(100, caster, 2, 0x5);

        subsystems.auras.add_owned(owned);
        subsystems.auras.add_applied(applied);
        subsystems.auras.set_visible(2, AuraRef::new(100, caster));
        subsystems.auras.mark_removed(AuraRef::new(100, caster));
        subsystems.auras.interrupt_flags = 0x10;
        subsystems.auras.interrupt_flags2 = 0x20;

        assert!(subsystems.auras.has_owned(owned));
        assert!(subsystems.auras.has_applied(applied));
        assert_eq!(
            subsystems.auras.visible_auras.get(&2).copied(),
            Some(AuraRef::new(100, caster))
        );
        assert_eq!(subsystems.auras.removed_count(), 1);
        assert!(subsystems.auras.remove_owned(owned));
        assert!(subsystems.auras.remove_applied(applied));
        assert_eq!(
            subsystems.auras.clear_visible(2),
            Some(AuraRef::new(100, caster))
        );
        subsystems.auras.clear_removed();
        assert_eq!(subsystems.auras.removed_count(), 0);

        let spell = CurrentSpellRef::new(200, Some(caster), Some(guid(3)));
        subsystems
            .spells
            .set_current_spell(CurrentSpellSlot::Generic, spell);
        assert_eq!(
            subsystems.spells.current_spell(CurrentSpellSlot::Generic),
            Some(spell)
        );
        assert_eq!(
            subsystems
                .spells
                .clear_current_spell(CurrentSpellSlot::Generic),
            Some(spell)
        );

        subsystems.spells.history.set_cooldown(200, 1_000, 30_000);
        subsystems.spells.history.set_charges(200, 2, 1_000, 10_000);
        assert_eq!(
            subsystems.spells.history.cooldown(200),
            Some(SpellCooldown {
                started_at_ms: 1_000,
                duration_ms: 30_000,
            })
        );
        assert_eq!(
            subsystems
                .spells
                .history
                .charges(200)
                .map(|state| state.charges),
            Some(2)
        );
        assert!(subsystems.spells.history.clear_cooldown(200));
        subsystems.spells.history.reset();
        assert!(subsystems.spells.history.cooldowns.is_empty());
        assert!(subsystems.spells.history.charges.is_empty());
    }

    #[test]
    fn current_spell_slots_match_trinity_values_and_roundtrip() {
        assert_eq!(CurrentSpellSlot::Melee as u8, 0);
        assert_eq!(CurrentSpellSlot::Generic as u8, 1);
        assert_eq!(CurrentSpellSlot::Channeled as u8, 2);
        assert_eq!(CurrentSpellSlot::Autorepeat as u8, 3);
        assert_eq!(CURRENT_FIRST_NON_MELEE_SPELL, 1);
        assert_eq!(CURRENT_MAX_SPELL, 4);

        let caster = guid(4);
        let mut spells = SpellSubsystem::default();
        let slots = [
            CurrentSpellSlot::Melee,
            CurrentSpellSlot::Generic,
            CurrentSpellSlot::Channeled,
            CurrentSpellSlot::Autorepeat,
        ];

        for (index, slot) in slots.into_iter().enumerate() {
            let spell = CurrentSpellRef::new(300 + index as u32, Some(caster), None);
            spells.set_current_spell(slot, spell);
            assert_eq!(spells.current_spell(slot), Some(spell));
            assert_eq!(spells.clear_current_spell(slot), Some(spell));
            assert_eq!(spells.current_spell(slot), None);
        }
    }

    #[test]
    fn threat_combat_helpers_roundtrip() {
        let mut combat = CombatSubsystem::default();
        let attacker = guid(10);

        assert!(!combat.combat_disallowed);
        assert_eq!(combat.add_threat(attacker, 5.0), 5.0);
        assert_eq!(combat.add_threat(attacker, 2.5), 7.5);
        assert!(combat.is_threatened_by(attacker));
        assert_eq!(combat.threat_value(attacker), Some(7.5));
        combat.set_threat(attacker, 1.0);
        assert_eq!(combat.remove_threat(attacker), Some(1.0));

        assert!(combat.add_attacker(attacker));
        combat.set_attacking(Some(attacker));
        combat.combat_disallowed = true;
        assert!(combat.attackers.contains(&attacker));
        assert_eq!(combat.attacking_guid, Some(attacker));
        assert!(combat.combat_disallowed);
        assert!(combat.remove_attacker(attacker));
        combat.clear_attackers();
        assert!(combat.attackers.is_empty());
        assert_eq!(combat.attacking_guid, None);
    }

    #[test]
    fn motion_charm_vehicle_and_ai_helpers_roundtrip() {
        let mut subsystems = UnitSubsystems::default();
        let controller = guid(20);
        let controlled = guid(21);
        let vehicle = guid(30);

        subsystems
            .motion
            .set_current_generator(MovementGeneratorKind::Chase);
        subsystems.motion.start_spline(7, 1_000);
        subsystems.motion.set_spline_progress(1_500);
        assert_eq!(
            subsystems.motion.current_generator,
            MovementGeneratorKind::Chase
        );
        assert_eq!(subsystems.motion.spline.progress_ms, 1_000);
        subsystems.motion.pause_movement();
        assert!(subsystems.motion.paused);
        subsystems.motion.resume_movement();
        subsystems.motion.stop_moving();
        assert!(!subsystems.motion.paused);
        assert!(subsystems.motion.stopped);
        assert!(!subsystems.motion.spline.enabled);

        subsystems.control.set_charmer(controller, true);
        subsystems.control.set_charmed(controlled);
        subsystems.control.unit_moved_by_me = Some(controlled);
        subsystems.control.player_moving_me = Some(controller);
        assert!(subsystems.control.is_charmed());
        assert!(subsystems.control.controlled_by_player);
        assert!(subsystems.control.controlled_guids.contains(&controlled));
        assert!(subsystems.control.add_shared_vision(controlled));
        subsystems.control.remove_charmed();
        subsystems.control.remove_charmer();
        assert!(!subsystems.control.is_charmed());
        assert_eq!(subsystems.control.last_charmer_guid, Some(controller));

        subsystems.vehicle.enter_vehicle(vehicle, Some(1));
        subsystems.vehicle.base_vehicle_guid = Some(vehicle);
        subsystems.vehicle.set_vehicle_kit(42, true);
        assert_eq!(subsystems.vehicle.vehicle_guid, Some(vehicle));
        assert_eq!(subsystems.vehicle.seat_id, Some(1));
        assert_eq!(subsystems.vehicle.kit.map(|kit| kit.kit_id), Some(42));
        subsystems.vehicle.exit_vehicle();
        subsystems.vehicle.clear_vehicle_kit();
        assert_eq!(subsystems.vehicle.vehicle_guid, None);
        assert_eq!(subsystems.vehicle.kit, None);

        subsystems.ai.set_active(Some("NullAI"));
        subsystems.ai.push("CombatAI");
        assert_eq!(subsystems.ai.active_ai.as_deref(), Some("CombatAI"));
        assert_eq!(subsystems.ai.ai_stack, vec![String::from("NullAI")]);
        assert_eq!(subsystems.ai.pop().as_deref(), Some("CombatAI"));
        assert_eq!(subsystems.ai.active_ai.as_deref(), Some("NullAI"));
        subsystems.ai.set_locked(true);
        assert!(subsystems.ai.locked);
    }
}
