# Class Info

Class info maps numeric **class IDs** to human-readable **network class names**. It
is the bridge between the integer identifiers used in the entity bit stream and the
serializer definitions that describe each class's fields.

## Structure

```
ClassInfo
  bits: 9              — number of bits needed to encode a class_id
  classes:
    [0] class_id: 0,   network_name: "CWorld",     table_name: "CWorld"
    [1] class_id: 5,   network_name: "CPlayer",    table_name: "CPlayer"
    ...
```

- **class_id**: integer identifier written into the entity bit stream during creation
- **network_name**: the class name used to look up the serializer
  (e.g., `"CCitadelPlayerController"`, `"CCitadelPlayerPawn"`)
- **table_name**: the send table name (typically the same as network_name)
- **bits**: `ceil(log2(max_class_id))` — determines how many bits the parser reads
  for the class ID when creating a new entity

## How It's Used

1. Parsed from the `DEM_ClassInfo` command during initialization
2. During entity creation, the parser reads `bits` bits from the entity data stream
   to get the `class_id`
3. The class ID is looked up in class info to get the `network_name`
4. The network name is used to find the corresponding serializer
5. The network name also becomes the entity's `class_name` field, which is how
   entities are filtered (e.g., `"CCitadelPlayerController"`)

## Relationship to Other Systems

```
class_id (from bit stream)
    → ClassInfo.by_id(class_id) → network_name
        → SerializerContainer.get(network_name) → Serializer
        → instance_baselines[class_id] → baseline field data
```

Class info is the central lookup that connects the compact integer encoding in the
wire format to the rich schema definitions in the serializer system.
