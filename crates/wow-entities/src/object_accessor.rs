use std::collections::HashMap;

use wow_constants::{TypeId, TypeMask};
use wow_core::ObjectGuid;
use wow_core::guid::HighGuid;

use crate::{Creature, GameObject, Item, Player, PlayerInventoryStorage, WorldObject};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AccessorObjectKind {
    Player,
    Creature,
    Pet,
    GameObject,
    Transport,
    DynamicObject,
    AreaTrigger,
    Corpse,
    SceneObject,
    Conversation,
}

impl AccessorObjectKind {
    pub fn from_guid(guid: ObjectGuid) -> Option<Self> {
        match guid.high_type() {
            HighGuid::Player => Some(Self::Player),
            HighGuid::Creature | HighGuid::Vehicle => Some(Self::Creature),
            HighGuid::Pet => Some(Self::Pet),
            HighGuid::GameObject => Some(Self::GameObject),
            HighGuid::Transport => Some(Self::Transport),
            HighGuid::DynamicObject => Some(Self::DynamicObject),
            HighGuid::AreaTrigger => Some(Self::AreaTrigger),
            HighGuid::Corpse => Some(Self::Corpse),
            HighGuid::SceneObject => Some(Self::SceneObject),
            HighGuid::Conversation => Some(Self::Conversation),
            _ => None,
        }
    }

    pub const fn type_mask(self) -> TypeMask {
        match self {
            Self::Player => TypeMask::PLAYER,
            Self::Creature | Self::Pet => TypeMask::UNIT,
            Self::GameObject | Self::Transport => TypeMask::GAME_OBJECT,
            Self::DynamicObject => TypeMask::DYNAMIC_OBJECT,
            Self::AreaTrigger => TypeMask::AREA_TRIGGER,
            Self::Corpse => TypeMask::CORPSE,
            Self::SceneObject => TypeMask::SCENE_OBJECT,
            Self::Conversation => TypeMask::CONVERSATION,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct AccessorPlayer {
    normalized_name: String,
    object: WorldObject,
    inventory: PlayerInventoryStorage,
    items: HashMap<ObjectGuid, Item>,
}

impl AccessorPlayer {
    pub fn new(name: impl AsRef<str>, object: WorldObject) -> Result<Self, ObjectAccessorError> {
        Self::new_with_inventory(name, object, PlayerInventoryStorage::default())
    }

    pub fn new_with_inventory(
        name: impl AsRef<str>,
        object: WorldObject,
        inventory: PlayerInventoryStorage,
    ) -> Result<Self, ObjectAccessorError> {
        Self::new_with_inventory_and_items(name, object, inventory, [])
    }

    pub fn new_with_inventory_and_items(
        name: impl AsRef<str>,
        object: WorldObject,
        inventory: PlayerInventoryStorage,
        items: impl IntoIterator<Item = Item>,
    ) -> Result<Self, ObjectAccessorError> {
        if !object.guid().is_player() {
            return Err(ObjectAccessorError::WrongGuidKind {
                guid: object.guid(),
                expected: AccessorObjectKind::Player,
            });
        }

        let normalized_name =
            normalize_player_name(name.as_ref()).ok_or(ObjectAccessorError::InvalidPlayerName)?;

        Ok(Self {
            normalized_name,
            object,
            inventory,
            items: items
                .into_iter()
                .map(|item| (item.object().guid(), item))
                .collect(),
        })
    }

    pub fn normalized_name(&self) -> &str {
        &self.normalized_name
    }

    pub const fn object(&self) -> &WorldObject {
        &self.object
    }

    pub fn object_mut(&mut self) -> &mut WorldObject {
        &mut self.object
    }

    pub const fn inventory(&self) -> &PlayerInventoryStorage {
        &self.inventory
    }

    pub fn inventory_mut(&mut self) -> &mut PlayerInventoryStorage {
        &mut self.inventory
    }

    pub fn item(&self, guid: ObjectGuid) -> Option<&Item> {
        self.items.get(&guid)
    }

    pub fn item_mut(&mut self, guid: ObjectGuid) -> Option<&mut Item> {
        self.items.get_mut(&guid)
    }

    pub fn insert_item(&mut self, item: Item) -> Option<Item> {
        self.items.insert(item.object().guid(), item)
    }

    pub fn remove_item(&mut self, guid: ObjectGuid) -> Option<Item> {
        self.items.remove(&guid)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AccessorObjectRef<'a> {
    WorldObject(&'a WorldObject),
    Item(&'a Item),
}

pub trait PlayerSaveSink {
    type Error;

    fn save_player(&mut self, player: &AccessorPlayer) -> Result<(), Self::Error>;
}

impl<F, E> PlayerSaveSink for F
where
    F: FnMut(&AccessorPlayer) -> Result<(), E>,
{
    type Error = E;

    fn save_player(&mut self, player: &AccessorPlayer) -> Result<(), Self::Error> {
        self(player)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayerSaveError<E> {
    pub guid: ObjectGuid,
    pub source: E,
}

pub trait ObjectAccessorMapSource {
    fn map_id(&self) -> u32;
    fn instance_id(&self) -> u32;
    fn map_object_record(&self, guid: ObjectGuid) -> Option<&MapObjectRecord>;
}

#[derive(Debug, Clone, PartialEq)]
pub struct MapObjectRecord {
    kind: AccessorObjectKind,
    body: MapObjectBody,
}

#[derive(Debug, Clone, PartialEq)]
enum MapObjectBody {
    WorldObject(WorldObject),
    Creature(Box<Creature>),
    GameObject(GameObject),
    Player(Box<Player>),
}

impl MapObjectRecord {
    pub fn new(kind: AccessorObjectKind, object: WorldObject) -> Result<Self, ObjectAccessorError> {
        if !object.has_current_map() {
            return Err(ObjectAccessorError::ObjectHasNoMap {
                guid: object.guid(),
            });
        }

        let guid_kind = AccessorObjectKind::from_guid(object.guid()).ok_or(
            ObjectAccessorError::UnsupportedGuidKind {
                guid: object.guid(),
            },
        )?;
        if !kind_accepts_guid(kind, guid_kind) {
            return Err(ObjectAccessorError::WrongGuidKind {
                guid: object.guid(),
                expected: kind,
            });
        }

        Ok(Self {
            kind,
            body: MapObjectBody::WorldObject(object),
        })
    }

    pub fn new_game_object(game_object: GameObject) -> Result<Self, ObjectAccessorError> {
        let record = Self::new(AccessorObjectKind::GameObject, game_object.world().clone())?;
        Ok(Self {
            kind: record.kind,
            body: MapObjectBody::GameObject(game_object),
        })
    }

    pub fn new_creature(creature: Creature) -> Result<Self, ObjectAccessorError> {
        let record = Self::new(
            AccessorObjectKind::Creature,
            creature.unit().world().clone(),
        )?;
        Ok(Self {
            kind: record.kind,
            body: MapObjectBody::Creature(Box::new(creature)),
        })
    }

    pub fn new_player(player: Player) -> Result<Self, ObjectAccessorError> {
        let record = Self::new(AccessorObjectKind::Player, player.unit().world().clone())?;
        Ok(Self {
            kind: record.kind,
            body: MapObjectBody::Player(Box::new(player)),
        })
    }

    pub const fn kind(&self) -> AccessorObjectKind {
        self.kind
    }

    pub fn object(&self) -> &WorldObject {
        match &self.body {
            MapObjectBody::WorldObject(object) => object,
            MapObjectBody::Creature(creature) => creature.unit().world(),
            MapObjectBody::GameObject(game_object) => game_object.world(),
            MapObjectBody::Player(player) => player.unit().world(),
        }
    }

    pub fn object_mut(&mut self) -> &mut WorldObject {
        match &mut self.body {
            MapObjectBody::WorldObject(object) => object,
            MapObjectBody::Creature(creature) => creature.unit_mut().world_mut(),
            MapObjectBody::GameObject(game_object) => game_object.world_mut(),
            MapObjectBody::Player(player) => player.unit_mut().world_mut(),
        }
    }

    pub fn creature(&self) -> Option<&Creature> {
        match &self.body {
            MapObjectBody::Creature(creature) => Some(creature.as_ref()),
            MapObjectBody::WorldObject(_)
            | MapObjectBody::GameObject(_)
            | MapObjectBody::Player(_) => None,
        }
    }

    pub fn creature_mut(&mut self) -> Option<&mut Creature> {
        match &mut self.body {
            MapObjectBody::Creature(creature) => Some(creature.as_mut()),
            MapObjectBody::WorldObject(_)
            | MapObjectBody::GameObject(_)
            | MapObjectBody::Player(_) => None,
        }
    }

    pub fn game_object(&self) -> Option<&GameObject> {
        match &self.body {
            MapObjectBody::GameObject(game_object) => Some(game_object),
            MapObjectBody::WorldObject(_)
            | MapObjectBody::Creature(_)
            | MapObjectBody::Player(_) => None,
        }
    }

    pub fn game_object_mut(&mut self) -> Option<&mut GameObject> {
        match &mut self.body {
            MapObjectBody::GameObject(game_object) => Some(game_object),
            MapObjectBody::WorldObject(_)
            | MapObjectBody::Creature(_)
            | MapObjectBody::Player(_) => None,
        }
    }

    pub fn player(&self) -> Option<&Player> {
        match &self.body {
            MapObjectBody::Player(player) => Some(player.as_ref()),
            MapObjectBody::WorldObject(_)
            | MapObjectBody::Creature(_)
            | MapObjectBody::GameObject(_) => None,
        }
    }

    pub fn player_mut(&mut self) -> Option<&mut Player> {
        match &mut self.body {
            MapObjectBody::Player(player) => Some(player.as_mut()),
            MapObjectBody::WorldObject(_)
            | MapObjectBody::Creature(_)
            | MapObjectBody::GameObject(_) => None,
        }
    }

    pub fn into_object(self) -> WorldObject {
        match self.body {
            MapObjectBody::WorldObject(object) => object,
            MapObjectBody::Creature(creature) => creature.unit().world().clone(),
            MapObjectBody::GameObject(game_object) => game_object.world().clone(),
            MapObjectBody::Player(player) => player.unit().world().clone(),
        }
    }
}

#[derive(Debug, Default)]
pub struct ObjectAccessor {
    players: HashMap<ObjectGuid, AccessorPlayer>,
    player_names: HashMap<String, ObjectGuid>,
}

impl ObjectAccessor {
    pub fn add_player(
        &mut self,
        name: impl AsRef<str>,
        object: WorldObject,
    ) -> Result<(), ObjectAccessorError> {
        self.add_player_with_inventory(name, object, PlayerInventoryStorage::default())
    }

    pub fn add_player_with_inventory(
        &mut self,
        name: impl AsRef<str>,
        object: WorldObject,
        inventory: PlayerInventoryStorage,
    ) -> Result<(), ObjectAccessorError> {
        self.add_player_with_inventory_and_items(name, object, inventory, [])
    }

    pub fn add_player_with_inventory_and_items(
        &mut self,
        name: impl AsRef<str>,
        object: WorldObject,
        inventory: PlayerInventoryStorage,
        items: impl IntoIterator<Item = Item>,
    ) -> Result<(), ObjectAccessorError> {
        let player = AccessorPlayer::new_with_inventory_and_items(name, object, inventory, items)?;
        let guid = player.object.guid();

        if let Some(previous) = self.players.insert(guid, player.clone()) {
            self.player_names.remove(previous.normalized_name());
        }
        self.player_names
            .insert(player.normalized_name.clone(), guid);

        Ok(())
    }

    pub fn player_inventory_mut(
        &mut self,
        guid: ObjectGuid,
    ) -> Option<&mut PlayerInventoryStorage> {
        self.players
            .get_mut(&guid)
            .map(AccessorPlayer::inventory_mut)
    }

    pub fn player_item(&self, player_guid: ObjectGuid, item_guid: ObjectGuid) -> Option<&Item> {
        self.players.get(&player_guid)?.item(item_guid)
    }

    pub fn player_item_mut(
        &mut self,
        player_guid: ObjectGuid,
        item_guid: ObjectGuid,
    ) -> Option<&mut Item> {
        self.players.get_mut(&player_guid)?.item_mut(item_guid)
    }

    pub fn insert_player_item(
        &mut self,
        player_guid: ObjectGuid,
        item: Item,
    ) -> Option<Option<Item>> {
        self.players
            .get_mut(&player_guid)
            .map(|player| player.insert_item(item))
    }

    pub fn remove_player_item(
        &mut self,
        player_guid: ObjectGuid,
        item_guid: ObjectGuid,
    ) -> Option<Item> {
        self.players.get_mut(&player_guid)?.remove_item(item_guid)
    }

    pub fn remove_player(&mut self, guid: ObjectGuid) -> Option<AccessorPlayer> {
        let removed = self.players.remove(&guid)?;
        self.player_names.remove(removed.normalized_name());
        Some(removed)
    }

    pub fn find_connected_player(&self, guid: ObjectGuid) -> Option<&WorldObject> {
        self.players.get(&guid).map(AccessorPlayer::object)
    }

    pub fn player_object_mut(&mut self, guid: ObjectGuid) -> Option<&mut WorldObject> {
        self.players.get_mut(&guid).map(AccessorPlayer::object_mut)
    }

    pub fn find_connected_player_by_name(&self, name: &str) -> Option<&WorldObject> {
        let normalized = normalize_player_name(name)?;
        let guid = self.player_names.get(&normalized)?;
        self.find_connected_player(*guid)
    }

    pub fn find_player(&self, guid: ObjectGuid) -> Option<&WorldObject> {
        self.find_connected_player(guid)
            .filter(|player| player.object().is_in_world())
    }

    pub fn find_player_by_name(&self, name: &str) -> Option<&WorldObject> {
        self.find_connected_player_by_name(name)
            .filter(|player| player.object().is_in_world())
    }

    pub fn find_player_by_low_guid(&self, low_guid: i64) -> Option<&WorldObject> {
        self.players
            .values()
            .find(|player| player.object.guid().counter() == low_guid)
            .map(AccessorPlayer::object)
            .filter(|player| player.object().is_in_world())
    }

    pub fn players(&self) -> impl Iterator<Item = (&ObjectGuid, &AccessorPlayer)> {
        self.players.iter()
    }

    pub fn save_all_players_with<F, E>(&self, mut save: F) -> Result<usize, PlayerSaveError<E>>
    where
        F: FnMut(&AccessorPlayer) -> Result<(), E>,
    {
        let mut saved = 0;
        for player in self.players.values() {
            save(player).map_err(|source| PlayerSaveError {
                guid: player.object().guid(),
                source,
            })?;
            saved += 1;
        }
        Ok(saved)
    }

    /// Legacy/test-only convenience mirroring the previous bridge helper.
    /// Use `save_all_players_with` when representing Trinity's real
    /// `ObjectAccessor::SaveAllPlayers()` behavior.
    pub fn save_all_players_count(&self) -> usize {
        self.players.len()
    }

    pub fn get_world_object(
        &self,
        context: &WorldObject,
        guid: ObjectGuid,
    ) -> Option<&WorldObject> {
        match AccessorObjectKind::from_guid(guid)? {
            AccessorObjectKind::Player => self.get_player(context, guid),
            AccessorObjectKind::Creature
            | AccessorObjectKind::Pet
            | AccessorObjectKind::GameObject
            | AccessorObjectKind::Transport
            | AccessorObjectKind::DynamicObject
            | AccessorObjectKind::AreaTrigger
            | AccessorObjectKind::Corpse
            | AccessorObjectKind::SceneObject
            | AccessorObjectKind::Conversation => None,
        }
    }

    pub fn get_object_by_type_mask(
        &self,
        context: &WorldObject,
        guid: ObjectGuid,
        type_mask: TypeMask,
    ) -> Option<&WorldObject> {
        match self.get_object_ref_by_type_mask(context, guid, type_mask)? {
            AccessorObjectRef::WorldObject(object) => Some(object),
            AccessorObjectRef::Item(_) => None,
        }
    }

    pub fn get_object_ref_by_type_mask(
        &self,
        context: &WorldObject,
        guid: ObjectGuid,
        type_mask: TypeMask,
    ) -> Option<AccessorObjectRef<'_>> {
        if guid.high_type() == HighGuid::Item {
            return self.get_item_ref_for_player_context(context, guid, type_mask);
        }

        match AccessorObjectKind::from_guid(guid)? {
            AccessorObjectKind::Player if type_mask.contains(TypeMask::PLAYER) => self
                .get_player(context, guid)
                .map(AccessorObjectRef::WorldObject),
            AccessorObjectKind::GameObject
            | AccessorObjectKind::Transport
            | AccessorObjectKind::Creature
            | AccessorObjectKind::Pet
            | AccessorObjectKind::DynamicObject
            | AccessorObjectKind::AreaTrigger
            | AccessorObjectKind::SceneObject
            | AccessorObjectKind::Conversation
            | AccessorObjectKind::Corpse => None,
            _ => None,
        }
    }

    pub fn get_world_object_from_map_source<'a, Source>(
        &'a self,
        context: &WorldObject,
        source: &'a Source,
        guid: ObjectGuid,
    ) -> Option<&'a WorldObject>
    where
        Source: ObjectAccessorMapSource + ?Sized,
    {
        match AccessorObjectKind::from_guid(guid)? {
            AccessorObjectKind::Player => self.get_player(context, guid),
            AccessorObjectKind::Creature => self.get_map_object_from_source(
                context,
                source,
                guid,
                &[AccessorObjectKind::Creature],
            ),
            AccessorObjectKind::Pet => {
                self.get_map_object_from_source(context, source, guid, &[AccessorObjectKind::Pet])
            }
            AccessorObjectKind::GameObject | AccessorObjectKind::Transport => self
                .get_map_object_from_source(
                    context,
                    source,
                    guid,
                    &[
                        AccessorObjectKind::GameObject,
                        AccessorObjectKind::Transport,
                    ],
                ),
            AccessorObjectKind::DynamicObject => self.get_map_object_from_source(
                context,
                source,
                guid,
                &[AccessorObjectKind::DynamicObject],
            ),
            AccessorObjectKind::AreaTrigger => self.get_map_object_from_source(
                context,
                source,
                guid,
                &[AccessorObjectKind::AreaTrigger],
            ),
            AccessorObjectKind::Corpse => self.get_map_object_from_source(
                context,
                source,
                guid,
                &[AccessorObjectKind::Corpse],
            ),
            AccessorObjectKind::SceneObject => self.get_map_object_from_source(
                context,
                source,
                guid,
                &[AccessorObjectKind::SceneObject],
            ),
            AccessorObjectKind::Conversation => self.get_map_object_from_source(
                context,
                source,
                guid,
                &[AccessorObjectKind::Conversation],
            ),
        }
    }

    pub fn get_object_ref_by_type_mask_from_map_source<'a, Source>(
        &'a self,
        context: &WorldObject,
        source: &'a Source,
        guid: ObjectGuid,
        type_mask: TypeMask,
    ) -> Option<AccessorObjectRef<'a>>
    where
        Source: ObjectAccessorMapSource + ?Sized,
    {
        if guid.high_type() == HighGuid::Item {
            return self.get_item_ref_for_player_context(context, guid, type_mask);
        }

        match AccessorObjectKind::from_guid(guid)? {
            AccessorObjectKind::Player if type_mask.contains(TypeMask::PLAYER) => self
                .get_player(context, guid)
                .map(AccessorObjectRef::WorldObject),
            AccessorObjectKind::GameObject | AccessorObjectKind::Transport
                if type_mask.contains(TypeMask::GAME_OBJECT) =>
            {
                self.get_map_object_from_source(
                    context,
                    source,
                    guid,
                    &[
                        AccessorObjectKind::GameObject,
                        AccessorObjectKind::Transport,
                    ],
                )
                .map(AccessorObjectRef::WorldObject)
            }
            AccessorObjectKind::Creature | AccessorObjectKind::Pet
                if type_mask.contains(TypeMask::UNIT) =>
            {
                self.get_map_object_from_source(
                    context,
                    source,
                    guid,
                    &[AccessorObjectKind::Creature, AccessorObjectKind::Pet],
                )
                .map(AccessorObjectRef::WorldObject)
            }
            AccessorObjectKind::DynamicObject if type_mask.contains(TypeMask::DYNAMIC_OBJECT) => {
                self.get_map_object_from_source(
                    context,
                    source,
                    guid,
                    &[AccessorObjectKind::DynamicObject],
                )
                .map(AccessorObjectRef::WorldObject)
            }
            AccessorObjectKind::AreaTrigger if type_mask.contains(TypeMask::AREA_TRIGGER) => self
                .get_map_object_from_source(
                    context,
                    source,
                    guid,
                    &[AccessorObjectKind::AreaTrigger],
                )
                .map(AccessorObjectRef::WorldObject),
            AccessorObjectKind::SceneObject if type_mask.contains(TypeMask::SCENE_OBJECT) => self
                .get_map_object_from_source(
                    context,
                    source,
                    guid,
                    &[AccessorObjectKind::SceneObject],
                )
                .map(AccessorObjectRef::WorldObject),
            AccessorObjectKind::Conversation if type_mask.contains(TypeMask::CONVERSATION) => self
                .get_map_object_from_source(
                    context,
                    source,
                    guid,
                    &[AccessorObjectKind::Conversation],
                )
                .map(AccessorObjectRef::WorldObject),
            AccessorObjectKind::Corpse => None,
            _ => None,
        }
    }

    pub fn get_unit_from_map_source<'a, Source>(
        &'a self,
        context: &WorldObject,
        source: &'a Source,
        guid: ObjectGuid,
    ) -> Option<&'a WorldObject>
    where
        Source: ObjectAccessorMapSource + ?Sized,
    {
        if guid.is_player() {
            return self.get_player(context, guid);
        }
        if guid.is_pet() {
            return self.get_pet_from_map_source(context, source, guid);
        }
        self.get_creature_from_map_source(context, source, guid)
    }

    pub fn get_creature_from_map_source<'a, Source>(
        &'a self,
        context: &WorldObject,
        source: &'a Source,
        guid: ObjectGuid,
    ) -> Option<&'a WorldObject>
    where
        Source: ObjectAccessorMapSource + ?Sized,
    {
        self.get_map_object_from_source(context, source, guid, &[AccessorObjectKind::Creature])
    }

    pub fn get_pet_from_map_source<'a, Source>(
        &'a self,
        context: &WorldObject,
        source: &'a Source,
        guid: ObjectGuid,
    ) -> Option<&'a WorldObject>
    where
        Source: ObjectAccessorMapSource + ?Sized,
    {
        self.get_map_object_from_source(context, source, guid, &[AccessorObjectKind::Pet])
    }

    pub fn get_creature_or_pet_or_vehicle_from_map_source<'a, Source>(
        &'a self,
        context: &WorldObject,
        source: &'a Source,
        guid: ObjectGuid,
    ) -> Option<&'a WorldObject>
    where
        Source: ObjectAccessorMapSource + ?Sized,
    {
        if guid.is_pet() {
            return self.get_pet_from_map_source(context, source, guid);
        }
        if guid.is_creature_or_vehicle() {
            return self.get_creature_from_map_source(context, source, guid);
        }
        None
    }

    pub fn get_game_object_from_map_source<'a, Source>(
        &'a self,
        context: &WorldObject,
        source: &'a Source,
        guid: ObjectGuid,
    ) -> Option<&'a WorldObject>
    where
        Source: ObjectAccessorMapSource + ?Sized,
    {
        self.get_map_object_from_source(
            context,
            source,
            guid,
            &[
                AccessorObjectKind::GameObject,
                AccessorObjectKind::Transport,
            ],
        )
    }

    pub fn get_corpse_from_map_source<'a, Source>(
        &'a self,
        context: &WorldObject,
        source: &'a Source,
        guid: ObjectGuid,
    ) -> Option<&'a WorldObject>
    where
        Source: ObjectAccessorMapSource + ?Sized,
    {
        self.get_map_object_from_source(context, source, guid, &[AccessorObjectKind::Corpse])
    }

    pub fn get_player(&self, context: &WorldObject, guid: ObjectGuid) -> Option<&WorldObject> {
        let player = self.find_connected_player(guid)?;
        player
            .object()
            .is_in_world()
            .then_some(player)
            .filter(|player| same_map(context, player))
    }

    #[deprecated(note = "map-local unit lookup requires ObjectAccessorMapSource")]
    pub fn get_unit(&self, context: &WorldObject, guid: ObjectGuid) -> Option<&WorldObject> {
        if guid.is_player() {
            return self.get_player(context, guid);
        }
        let _ = (context, guid);
        None
    }

    #[deprecated(note = "map-local creature lookup requires ObjectAccessorMapSource")]
    pub fn get_creature(&self, context: &WorldObject, guid: ObjectGuid) -> Option<&WorldObject> {
        let _ = (context, guid);
        None
    }

    #[deprecated(note = "map-local pet lookup requires ObjectAccessorMapSource")]
    pub fn get_pet(&self, context: &WorldObject, guid: ObjectGuid) -> Option<&WorldObject> {
        let _ = (context, guid);
        None
    }

    #[deprecated(note = "map-local creature/pet/vehicle lookup requires ObjectAccessorMapSource")]
    pub fn get_creature_or_pet_or_vehicle(
        &self,
        context: &WorldObject,
        guid: ObjectGuid,
    ) -> Option<&WorldObject> {
        let _ = (context, guid);
        None
    }

    #[deprecated(note = "map-local gameobject lookup requires ObjectAccessorMapSource")]
    pub fn get_game_object(&self, context: &WorldObject, guid: ObjectGuid) -> Option<&WorldObject> {
        let _ = (context, guid);
        None
    }

    #[deprecated(note = "map-local transport lookup requires ObjectAccessorMapSource")]
    pub fn get_transport(&self, context: &WorldObject, guid: ObjectGuid) -> Option<&WorldObject> {
        let _ = (context, guid);
        None
    }

    #[deprecated(note = "map-local dynamic object lookup requires ObjectAccessorMapSource")]
    pub fn get_dynamic_object(
        &self,
        context: &WorldObject,
        guid: ObjectGuid,
    ) -> Option<&WorldObject> {
        let _ = (context, guid);
        None
    }

    #[deprecated(note = "map-local area trigger lookup requires ObjectAccessorMapSource")]
    pub fn get_area_trigger(
        &self,
        context: &WorldObject,
        guid: ObjectGuid,
    ) -> Option<&WorldObject> {
        let _ = (context, guid);
        None
    }

    #[deprecated(note = "map-local corpse lookup requires ObjectAccessorMapSource")]
    pub fn get_corpse(&self, context: &WorldObject, guid: ObjectGuid) -> Option<&WorldObject> {
        let _ = (context, guid);
        None
    }

    #[deprecated(note = "map-local scene object lookup requires ObjectAccessorMapSource")]
    pub fn get_scene_object(
        &self,
        context: &WorldObject,
        guid: ObjectGuid,
    ) -> Option<&WorldObject> {
        let _ = (context, guid);
        None
    }

    #[deprecated(note = "map-local conversation lookup requires ObjectAccessorMapSource")]
    pub fn get_conversation(
        &self,
        context: &WorldObject,
        guid: ObjectGuid,
    ) -> Option<&WorldObject> {
        let _ = (context, guid);
        None
    }

    fn get_map_object_from_source<'a, Source>(
        &'a self,
        context: &WorldObject,
        source: &'a Source,
        guid: ObjectGuid,
        allowed: &[AccessorObjectKind],
    ) -> Option<&'a WorldObject>
    where
        Source: ObjectAccessorMapSource + ?Sized,
    {
        if !context.has_current_map()
            || context.map_id() != source.map_id()
            || context.instance_id() != source.instance_id()
        {
            return None;
        }

        let record = source.map_object_record(guid)?;
        if !allowed.contains(&record.kind()) || !same_map(context, record.object()) {
            return None;
        }

        Some(record.object())
    }

    fn get_item_ref_for_player_context(
        &self,
        context: &WorldObject,
        guid: ObjectGuid,
        type_mask: TypeMask,
    ) -> Option<AccessorObjectRef<'_>> {
        if !type_mask.contains(TypeMask::ITEM) || context.object().type_id() != TypeId::Player {
            return None;
        }

        let player = self.players.get(&context.guid())?;
        let item_guid = player.inventory().get_item_by_guid_everywhere(guid)?;
        player.item(item_guid).map(AccessorObjectRef::Item)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectAccessorError {
    InvalidPlayerName,
    UnsupportedGuidKind {
        guid: ObjectGuid,
    },
    WrongGuidKind {
        guid: ObjectGuid,
        expected: AccessorObjectKind,
    },
    ObjectHasNoMap {
        guid: ObjectGuid,
    },
}

pub fn normalize_player_name(name: &str) -> Option<String> {
    let mut chars = name.chars();
    let first = chars.next()?;
    let mut normalized = String::new();
    normalized.extend(first.to_uppercase());
    for ch in chars {
        normalized.extend(ch.to_lowercase());
    }
    Some(normalized)
}

fn same_map(left: &WorldObject, right: &WorldObject) -> bool {
    left.has_current_map()
        && right.has_current_map()
        && left.map_id() == right.map_id()
        && left.instance_id() == right.instance_id()
}

fn kind_accepts_guid(kind: AccessorObjectKind, guid_kind: AccessorObjectKind) -> bool {
    kind == guid_kind
}

#[cfg(test)]
mod tests {
    use super::*;
    use wow_constants::{TypeId, TypeMask};
    use wow_core::Position;

    fn guid(high: HighGuid, counter: i64) -> ObjectGuid {
        if high == HighGuid::Player {
            ObjectGuid::create_global(high, 0, counter)
        } else if high == HighGuid::Transport {
            ObjectGuid::create_transport(high, counter)
        } else {
            ObjectGuid::create_world_object(high, 0, 1, 530, 1, 100, counter)
        }
    }

    fn world_object(high: HighGuid, map_id: u32, instance_id: u32, in_world: bool) -> WorldObject {
        let type_id = guid(high, 1).type_id();
        let type_mask = match type_id {
            wow_core::guid::TypeId::Player => TypeMask::PLAYER,
            wow_core::guid::TypeId::Unit => TypeMask::UNIT,
            wow_core::guid::TypeId::GameObject => TypeMask::GAME_OBJECT,
            wow_core::guid::TypeId::DynamicObject => TypeMask::DYNAMIC_OBJECT,
            wow_core::guid::TypeId::Corpse => TypeMask::CORPSE,
            wow_core::guid::TypeId::AreaTrigger => TypeMask::AREA_TRIGGER,
            wow_core::guid::TypeId::SceneObject => TypeMask::SCENE_OBJECT,
            wow_core::guid::TypeId::Conversation => TypeMask::CONVERSATION,
            _ => TypeMask::OBJECT,
        };
        let mut object = WorldObject::new(false, convert_type_id(type_id), type_mask);
        object.object_mut().create(guid(high, 1));
        object.set_map(map_id, instance_id).unwrap();
        object.relocate(Position::xyz(1.0, 2.0, 3.0));
        if in_world {
            object.object_mut().add_to_world();
        }
        object
    }

    fn item(guid: ObjectGuid, entry: u32) -> Item {
        let mut item = Item::default();
        item.object_mut().create(guid);
        item.object_mut().set_entry(entry);
        item
    }

    fn convert_type_id(type_id: wow_core::guid::TypeId) -> TypeId {
        match type_id {
            wow_core::guid::TypeId::Object => TypeId::Object,
            wow_core::guid::TypeId::Item => TypeId::Item,
            wow_core::guid::TypeId::Container => TypeId::Container,
            wow_core::guid::TypeId::AzeriteEmpoweredItem => TypeId::AzeriteEmpoweredItem,
            wow_core::guid::TypeId::AzeriteItem => TypeId::AzeriteItem,
            wow_core::guid::TypeId::Unit => TypeId::Unit,
            wow_core::guid::TypeId::Player => TypeId::Player,
            wow_core::guid::TypeId::ActivePlayer => TypeId::ActivePlayer,
            wow_core::guid::TypeId::GameObject => TypeId::GameObject,
            wow_core::guid::TypeId::DynamicObject => TypeId::DynamicObject,
            wow_core::guid::TypeId::Corpse => TypeId::Corpse,
            wow_core::guid::TypeId::AreaTrigger => TypeId::AreaTrigger,
            wow_core::guid::TypeId::SceneObject => TypeId::SceneObject,
            wow_core::guid::TypeId::Conversation => TypeId::Conversation,
        }
    }

    #[test]
    fn player_name_normalization_matches_cpp_shape() {
        assert_eq!(normalize_player_name("thrall"), Some("Thrall".to_string()));
        assert_eq!(normalize_player_name("THRALL"), Some("Thrall".to_string()));
        assert_eq!(normalize_player_name(""), None);
    }

    #[test]
    fn global_player_lookup_distinguishes_connected_and_in_world() {
        let mut accessor = ObjectAccessor::default();
        let player = world_object(HighGuid::Player, 1, 0, false);
        let player_guid = player.guid();

        accessor.add_player("jaina", player).unwrap();
        assert!(accessor.find_connected_player(player_guid).is_some());
        assert!(accessor.find_connected_player_by_name("JAINA").is_some());
        assert!(accessor.find_player(player_guid).is_none());
        assert!(accessor.find_player_by_name("jaina").is_none());

        let mut in_world = world_object(HighGuid::Player, 1, 0, true);
        in_world.object_mut().create(player_guid);
        accessor.add_player("jaina", in_world).unwrap();
        assert!(accessor.find_player(player_guid).is_some());
        assert_eq!(accessor.save_all_players_count(), 1);
    }

    #[test]
    fn map_object_record_can_store_typed_gameobject_like_cpp() {
        let mut game_object = GameObject::new();
        let guid = guid(HighGuid::GameObject, 77);
        game_object.world_mut().object_mut().create(guid);
        game_object.world_mut().object_mut().set_entry(123);
        game_object.world_mut().set_map(571, 0).unwrap();
        game_object
            .world_mut()
            .relocate(Position::xyz(1.0, 2.0, 3.0));
        game_object.set_created_by(ObjectGuid::create_player(1, 42));

        let mut record = MapObjectRecord::new_game_object(game_object).unwrap();

        assert_eq!(record.kind(), AccessorObjectKind::GameObject);
        assert_eq!(record.object().guid(), guid);
        assert_eq!(
            record.game_object().unwrap().owner_guid(),
            ObjectGuid::create_player(1, 42)
        );
        record.object_mut().relocate(Position::xyz(4.0, 5.0, 6.0));
        assert_eq!(record.game_object().unwrap().world().position().x, 4.0);
    }

    #[test]
    fn map_object_record_can_store_typed_creature_like_cpp() {
        let mut creature = Creature::new(false);
        let guid = guid(HighGuid::Creature, 78);
        creature.unit_mut().world_mut().object_mut().create(guid);
        creature.unit_mut().world_mut().object_mut().set_entry(321);
        creature.unit_mut().world_mut().set_map(571, 0).unwrap();
        creature
            .unit_mut()
            .world_mut()
            .relocate(Position::xyz(1.0, 2.0, 3.0));
        creature.unit_mut().set_level(42);

        let mut record = MapObjectRecord::new_creature(creature).unwrap();

        assert_eq!(record.kind(), AccessorObjectKind::Creature);
        assert_eq!(record.object().guid(), guid);
        assert_eq!(record.creature().unwrap().unit().data().level, 42);
        record
            .creature_mut()
            .unwrap()
            .unit_mut()
            .world_mut()
            .relocate(Position::xyz(4.0, 5.0, 6.0));
        assert_eq!(record.object().position().x, 4.0);
    }

    #[test]
    fn map_object_record_can_store_typed_player_like_cpp() {
        let mut player = Player::new(Some(7), false);
        let player_guid = ObjectGuid::create_player(1, 42);
        let victim_guid = guid(HighGuid::Creature, 77);
        player
            .unit_mut()
            .world_mut()
            .object_mut()
            .create(player_guid);
        player.unit_mut().world_mut().set_map(571, 7).unwrap();
        player
            .unit_mut()
            .world_mut()
            .relocate(Position::xyz(1.0, 2.0, 3.0));
        player.unit_mut().set_attacking(Some(victim_guid));

        let mut record = MapObjectRecord::new_player(player).unwrap();

        assert_eq!(record.kind(), AccessorObjectKind::Player);
        assert_eq!(record.object().guid(), player_guid);
        assert_eq!(
            record.player().unwrap().unit().attacking(),
            Some(victim_guid)
        );
        record.player_mut().unwrap().unit_mut().set_attacking(None);
        assert_eq!(record.player().unwrap().unit().attacking(), None);
    }

    #[test]
    fn save_all_players_with_invokes_sink_once_per_registered_player() {
        let mut accessor = ObjectAccessor::default();
        let player_a = world_object(HighGuid::Player, 1, 0, true);
        let guid_a = player_a.guid();
        let mut player_b = world_object(HighGuid::Player, 1, 0, true);
        player_b
            .object_mut()
            .create(ObjectGuid::create_global(HighGuid::Player, 0, 2));
        let guid_b = player_b.guid();

        accessor.add_player("jaina", player_a).unwrap();
        accessor.add_player("thrall", player_b).unwrap();

        let mut saved = Vec::new();
        let count = accessor
            .save_all_players_with(|player| {
                saved.push(player.object().guid());
                Ok::<(), ()>(())
            })
            .unwrap();

        assert_eq!(count, 2);
        assert!(saved.contains(&guid_a));
        assert!(saved.contains(&guid_b));
        assert_eq!(saved.len(), 2);
    }

    #[test]
    fn save_all_players_with_propagates_error_with_player_guid() {
        let mut accessor = ObjectAccessor::default();
        let player = world_object(HighGuid::Player, 1, 0, true);
        let guid = player.guid();
        accessor.add_player("jaina", player).unwrap();

        let error = accessor
            .save_all_players_with(|_| Err::<(), _>("db unavailable"))
            .unwrap_err();

        assert_eq!(error.guid, guid);
        assert_eq!(error.source, "db unavailable");
    }

    #[test]
    fn save_all_players_with_does_not_break_name_or_canonical_map_source_lookup() {
        let mut accessor = ObjectAccessor::default();
        let context = world_object(HighGuid::Player, 530, 1, true);
        let player_guid = context.guid();
        let creature = world_object(HighGuid::Creature, 530, 1, true);
        let creature_guid = creature.guid();
        let record = MapObjectRecord::new(AccessorObjectKind::Creature, creature).unwrap();
        let mut source = TestMapSource {
            map_id: 530,
            instance_id: 1,
            records: std::collections::HashMap::new(),
        };
        source.records.insert(creature_guid, record);

        accessor.add_player("valeera", context.clone()).unwrap();

        let saved = accessor
            .save_all_players_with(|_| Ok::<(), ()>(()))
            .unwrap();

        assert_eq!(saved, 1);
        assert_eq!(
            accessor
                .find_connected_player_by_name("VALEERA")
                .unwrap()
                .guid(),
            player_guid
        );
        assert_eq!(
            accessor
                .get_creature_from_map_source(&context, &source, creature_guid)
                .unwrap()
                .guid(),
            creature_guid
        );
    }

    #[derive(Default)]
    struct TestMapSource {
        map_id: u32,
        instance_id: u32,
        records: std::collections::HashMap<ObjectGuid, MapObjectRecord>,
    }

    impl ObjectAccessorMapSource for TestMapSource {
        fn map_id(&self) -> u32 {
            self.map_id
        }

        fn instance_id(&self) -> u32 {
            self.instance_id
        }

        fn map_object_record(&self, guid: ObjectGuid) -> Option<&MapObjectRecord> {
            self.records.get(&guid)
        }
    }

    #[test]
    fn map_source_lookup_reads_canonical_source_without_bridge_storage() {
        let accessor = ObjectAccessor::default();
        let context = world_object(HighGuid::Player, 530, 1, true);
        let creature = world_object(HighGuid::Creature, 530, 1, true);
        let creature_guid = creature.guid();
        let record = MapObjectRecord::new(AccessorObjectKind::Creature, creature).unwrap();
        let mut source = TestMapSource {
            map_id: 530,
            instance_id: 1,
            records: std::collections::HashMap::new(),
        };
        source.records.insert(creature_guid, record);

        assert_eq!(
            accessor
                .get_world_object_from_map_source(&context, &source, creature_guid)
                .unwrap()
                .guid(),
            creature_guid
        );
        assert!(matches!(
            accessor.get_object_ref_by_type_mask_from_map_source(
                &context,
                &source,
                creature_guid,
                TypeMask::UNIT
            ),
            Some(AccessorObjectRef::WorldObject(object)) if object.guid() == creature_guid
        ));

        source.instance_id = 2;
        assert!(
            accessor
                .get_world_object_from_map_source(&context, &source, creature_guid)
                .is_none()
        );
    }

    #[test]
    fn get_player_requires_same_map_like_cpp_get_player_map() {
        let mut accessor = ObjectAccessor::default();
        let context = world_object(HighGuid::Creature, 1, 0, true);
        let same_map_player = world_object(HighGuid::Player, 1, 0, true);
        let same_guid = same_map_player.guid();
        let mut other_map_player = world_object(HighGuid::Player, 2, 0, true);
        other_map_player
            .object_mut()
            .create(ObjectGuid::create_global(HighGuid::Player, 0, 2));
        let other_guid = other_map_player.guid();

        accessor.add_player("anduin", same_map_player).unwrap();
        accessor.add_player("baine", other_map_player).unwrap();

        assert!(accessor.get_player(&context, same_guid).is_some());
        assert!(accessor.get_player(&context, other_guid).is_none());
    }

    #[test]
    fn world_object_dispatches_by_high_guid_to_map_source() {
        let accessor = ObjectAccessor::default();
        let context = world_object(HighGuid::Player, 530, 1, true);
        let creature = world_object(HighGuid::Creature, 530, 1, true);
        let gameobject = world_object(HighGuid::GameObject, 530, 1, true);
        let creature_guid = creature.guid();
        let gameobject_guid = gameobject.guid();
        let mut source = TestMapSource {
            map_id: 530,
            instance_id: 1,
            records: std::collections::HashMap::new(),
        };

        source.records.insert(
            creature_guid,
            MapObjectRecord::new(AccessorObjectKind::Creature, creature).unwrap(),
        );
        source.records.insert(
            gameobject_guid,
            MapObjectRecord::new(AccessorObjectKind::GameObject, gameobject).unwrap(),
        );

        assert_eq!(
            accessor
                .get_world_object_from_map_source(&context, &source, creature_guid)
                .unwrap()
                .guid(),
            creature_guid
        );
        assert_eq!(
            accessor
                .get_world_object_from_map_source(&context, &source, gameobject_guid)
                .unwrap()
                .guid(),
            gameobject_guid
        );
    }

    #[test]
    fn object_by_type_mask_matches_cpp_dispatch_rules_with_map_source() {
        let accessor = ObjectAccessor::default();
        let context = world_object(HighGuid::Player, 530, 1, true);
        let creature = world_object(HighGuid::Creature, 530, 1, true);
        let creature_guid = creature.guid();
        let mut source = TestMapSource {
            map_id: 530,
            instance_id: 1,
            records: std::collections::HashMap::new(),
        };
        source.records.insert(
            creature_guid,
            MapObjectRecord::new(AccessorObjectKind::Creature, creature).unwrap(),
        );

        assert!(
            accessor
                .get_object_ref_by_type_mask_from_map_source(
                    &context,
                    &source,
                    creature_guid,
                    TypeMask::UNIT
                )
                .is_some()
        );
        assert!(
            accessor
                .get_object_ref_by_type_mask_from_map_source(
                    &context,
                    &source,
                    creature_guid,
                    TypeMask::GAME_OBJECT
                )
                .is_none()
        );
        assert!(
            accessor
                .get_object_ref_by_type_mask_from_map_source(
                    &context,
                    &source,
                    creature_guid,
                    TypeMask::PLAYER
                )
                .is_none()
        );
    }

    #[test]
    fn type_mask_item_uses_player_inventory_like_cpp_branch() {
        let mut accessor = ObjectAccessor::default();
        let context = world_object(HighGuid::Player, 530, 1, true);
        let player_guid = context.guid();
        let item_guid = ObjectGuid::create_item(1, 77);
        let mut inventory = PlayerInventoryStorage::default();
        inventory.items[0] = Some(item_guid);
        let item = item(item_guid, 6948);

        accessor
            .add_player_with_inventory_and_items("valeera", context.clone(), inventory, [item])
            .unwrap();

        let found = accessor.get_object_ref_by_type_mask(&context, item_guid, TypeMask::ITEM);
        match found {
            Some(AccessorObjectRef::Item(item)) => {
                assert_eq!(item.object().guid(), item_guid);
                assert_eq!(item.object().entry(), 6948);
            }
            other => panic!("expected item ref, got {other:?}"),
        }
        assert!(
            accessor
                .get_object_by_type_mask(&context, item_guid, TypeMask::ITEM)
                .is_none()
        );
        assert!(
            accessor
                .get_object_ref_by_type_mask(&context, item_guid, TypeMask::UNIT)
                .is_none()
        );

        let non_player_context = world_object(HighGuid::Creature, 530, 1, true);
        assert!(
            accessor
                .get_object_ref_by_type_mask(&non_player_context, item_guid, TypeMask::ITEM)
                .is_none()
        );
        assert!(accessor.player_inventory_mut(player_guid).is_some());
    }

    #[test]
    fn type_mask_item_requires_registered_item_object_like_cpp_item_pointer() {
        let mut accessor = ObjectAccessor::default();
        let context = world_object(HighGuid::Player, 530, 1, true);
        let item_guid = ObjectGuid::create_item(1, 77);
        let mut inventory = PlayerInventoryStorage::default();
        inventory.items[0] = Some(item_guid);

        accessor
            .add_player_with_inventory("valeera", context.clone(), inventory)
            .unwrap();

        assert!(
            accessor
                .get_object_ref_by_type_mask(&context, item_guid, TypeMask::ITEM)
                .is_none()
        );
    }

    #[test]
    fn corpse_is_directly_accessible_but_not_returned_by_type_mask_like_cpp() {
        let accessor = ObjectAccessor::default();
        let context = world_object(HighGuid::Player, 530, 1, true);
        let corpse = world_object(HighGuid::Corpse, 530, 1, true);
        let corpse_guid = corpse.guid();
        let mut source = TestMapSource {
            map_id: 530,
            instance_id: 1,
            records: std::collections::HashMap::new(),
        };

        source.records.insert(
            corpse_guid,
            MapObjectRecord::new(AccessorObjectKind::Corpse, corpse).unwrap(),
        );

        assert_eq!(
            accessor
                .get_corpse_from_map_source(&context, &source, corpse_guid)
                .unwrap()
                .guid(),
            corpse_guid
        );
        assert!(
            accessor
                .get_world_object_from_map_source(&context, &source, corpse_guid)
                .is_some()
        );
        assert!(
            accessor
                .get_object_ref_by_type_mask_from_map_source(
                    &context,
                    &source,
                    corpse_guid,
                    TypeMask::CORPSE
                )
                .is_none()
        );
    }

    #[test]
    fn unit_and_creature_or_pet_or_vehicle_helpers_match_cpp() {
        let accessor = ObjectAccessor::default();
        let context = world_object(HighGuid::Player, 530, 1, true);
        let pet = world_object(HighGuid::Pet, 530, 1, true);
        let pet_guid = pet.guid();
        let mut source = TestMapSource {
            map_id: 530,
            instance_id: 1,
            records: std::collections::HashMap::new(),
        };
        source.records.insert(
            pet_guid,
            MapObjectRecord::new(AccessorObjectKind::Pet, pet).unwrap(),
        );

        assert_eq!(
            accessor
                .get_unit_from_map_source(&context, &source, pet_guid)
                .unwrap()
                .guid(),
            pet_guid
        );
        assert_eq!(
            accessor
                .get_creature_or_pet_or_vehicle_from_map_source(&context, &source, pet_guid)
                .unwrap()
                .guid(),
            pet_guid
        );
    }
}
