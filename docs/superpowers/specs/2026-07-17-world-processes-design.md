# World Processes — 自主过程机制设计

**Date:** 2026-07-17
**Status:** Approved (design), pending implementation plan
**Principle:** **只写简单规则，不写手工结局。** 火把、护城河、防火带、烧怪、农场都不是功能，是几条小规则相互作用产生的读数。This is the design's constitution: every mechanic below is a simple flag-driven rule; gameplay richness must be traceable to rule interactions, never to hand-authored special cases.

---

## 1. 目标与范围

让世界从"只有 agent 编辑才变化"变成"自己会演化"：水会漫、火会烧、草会长，且全部过程遵守两条铁律：

1. **物质守恒**（ARCHITECTURE rule 7）：任何过程链不得净产出任何 matter kind。
2. **物品认知更少、组合效果更多**：不新增物品类型/物品本体论；新行为全部来自 f_info flags（tree/brake/water/lava/walk）与现有 Matter 的重新组合。

### In

- 过程引擎（`systems/process.rs`）+ 纯数据规则表（`process_rules.rs`）
- 火（蔓延/熄灭/水克）+ 水（漫延/摊薄）+ 草（蔓延）三类过程
- 通用 `use` 动词（可燃块点火、有机块食用）
- FIRE feat（f_info N:99）+ 动态 Glow（火光照亮）
- TerrainChanged 事件（走现有 FOV 门控）

### Out（YAGNI）

- 风/天气/季节、烟雾、怪物避火 AI（怪物走既有地形规则，会死在火里——已足够）
- 树自我复制（物质守恒不允许，见 §7）
- 新物品类型/装备系统/化学属性模型（方案 C，留作未来）
- 第二类建筑实体（build 动词保持只做 hut）
- agent 组织层（org/skill/memory——玩家自组织领域，不碰）

---

## 2. 架构落点

完全贴合现有分层（docs/ARCHITECTURE.md）：

```
process_rules.rs   纯数据规则表（零 ECS，同 sandbox.rs 哲学）
systems/process.rs 引擎（规则表 → Grid/GlowMask/EventBuf）
tick.rs            check_deaths 之后、PendingLevelChange 之前插入 process_world
balance.rs         全部速率（PROCESS_EVERY_N、各规则概率）
```

- 加过程 = 加一行表，不改引擎。
- 概率判定用 `hash(world_seed ^ tick ^ cell_idx)`，同 seed 全复现（模拟确定性不破）。
- 性能：`PROCESS_EVERY_N = 8`（250ms×8 ≈ 2s/过程 tick）。先按 flag/id 预筛候选格（72k 格一次 flag 扫），每过程 tick 实际处理预期 <500 格。

---

## 3. 规则表形状（本机制的全部概念）

```rust
pub enum CellCond {
    FeatIs(FeatId),
    Flag(&'static str),        // "tree" | "water" | "lava" | "walk" | "brake" …
}

pub enum NeighborCond {
    None,
    AnyFlag(&'static str),     // 四邻任一匹配 flag
    AnyFeat(FeatId),           // 四邻任一是 feat
    NoFlammable,               // 四邻无可燃（tree/brake flag 或 door flag）
}

pub enum ProcessAction {
    NeighborBecomes(FeatId),              // 选中一个符合条件的邻居→feat
    SelfBecomes(FeatId),                  // 自身→feat
    SelfBecomesOneOf(&'static [(FeatId, u8)]),  // 加权随机（如 FLOOR 60/RUBBLE 40）
    NeighborAndSelf { neighbor: FeatId, self_: Option<(FeatId, u8)> }, // 邻居→feat，自身按概率→feat（水漫延摊薄）
}

pub struct ProcessRule {
    pub name: &'static str,
    pub on: CellCond,
    pub neighbors: NeighborCond,
    pub action: ProcessAction,
    pub chance_pct: u8,         // 每过程 tick 触发概率（deterministic hash）
    pub cause: Cause,           // Fire | Water | Growth（事件归因 + 测试断言用）
}
```

**规则作用于 flag 与少量 feat id，不作用于枚举清单。** 将来 f_info 新增任何带 tree flag 的植物，火自动烧它——零代码。

引擎：`systems/process.rs::process_world(world)`
1. `tick % PROCESS_EVERY_N != 0` → return
2. 预筛：按每条规则的 `on` 收集候选格
3. 每候选格：检查 `neighbors`，按 `chance_pct` 掷 `hash(seed^tick^idx) % 100`
4. 命中→应用 `action` 到 Grid，push `GameEvent::TerrainChanged { at, from, to, cause }`
5. 应用顺序按表序（火先水后草），单 tick 内同格只被一个规则变换（已变换格跳过）

---

## 4. 全部规则（七行，就是本轮全部游戏设计）

| # | name | on | neighbors | action | % | 读出 |
|---|------|----|-----------|--------|---|------|
| 1 | fire_spread | FeatIs(FIRE) | AnyFlag(tree/brake) 或 door flag | NeighborBecomes(FIRE) | 15 | 火烧森林/木门 |
| 2 | fire_burnout | FeatIs(FIRE) | None | SelfBecomesOneOf(FLOOR 90, RUBBLE 10) | 6 | 野火自然熄灭成灰 |
| 3 | fire_douse | Flag(water) | AnyFeat(FIRE) | NeighborBecomes(FLOOR) | 100 | 水克火（护城河涌现） |
| 4 | water_evaporate | FeatIs(SHALLOW_WATER) | AnyFeat(FIRE) | SelfBecomes(FLOOR) | 20 | 浅水被火烤干 |
| 5 | water_flow_deep | FeatIs(DEEP_WATER) | AnyFlag(walk) 且非水非门¹ | NeighborBecomes(SHALLOW_WATER) | 2 | 深水是水源 |
| 6 | water_flow_shallow | FeatIs(SHALLOW_WATER) | 同上¹ | NeighborAndSelf(SHALLOW_WATER, Some(FLOOR, 100)) | 8 | 浅水漫延并摊薄（守恒） |
| 7 | grass_spread | FeatIs(GRASS) | AnyFeat(DIRT)，且曼哈顿距离 ≤3 内有水 flag | NeighborBecomes(GRASS) | 8 | 草随水走（农场涌现） |

¹ "非水非门" = 邻居格不带 water flag、不带 door flag，且 walk。

编号 5-6 即"水漫延"拆成两条简单规则：深水不消耗是水源，浅水移动即摊薄——水总量守恒。深水产生速率(2%)远低于浅水摊薄速率(8%)，水平衡全局收缩，无印钞。

**涌现读数（零代码）：**
- 森林放火→火烧连营；怪物走同一套地形规则（FIRE 带 LAVA flag→on_enter_cell 岩浆伤害分支），**火会烧死怪**
- 护城河=水克火；防火带=提前烧出隔离带或挖空燃料
- **火把=浅岩浆块+不可燃基座**：浅岩浆不蔓延不熄灭（规则不管它），置于花岗岩基座上即成长明灯——已有机制的读数，不是新功能
- 灰烬 RUBBLE 可挖可压回花岗岩，火后重建有材料

---

## 5. use 动词（一个动词，按块类型分发）

`systems/verbs.rs` 注册表加一行 `use { slot }`（priority 14），实现在 `systems/use_item.rs`：

| 块的判定（f_info flags） | 效果 | 消耗 |
|---|---|---|
| 可燃块（TREE feat 块 / Matter::Resource wood） | 目标格（interact 规则：脚下或相邻）→FIRE。目标须 walk 或带 tree/brake/door flag；自身脚下亦可（会受伤，允许）。事件 TerrainChanged(cause: Fire) | 1 块 |
| 有机块（GRASS/BRAKE feat 块） | 吃掉：hp+1（不超过 max）。事件 `GameEvent::Consumed { entity, label, hp }` | 1 块 |
| 其他 | ActionRejected `not_usable` | — |

判定全走 flags，k_info 装饰物将来挂同一个分发点。**不新增物品类型。**

---

## 6. FIRE feat 与动态 Glow

f_info.txt 追加：

```
N:99:FIRE
E:spreading fire
G:!:r
F:WALK | LAVA | LIT
```

- `WALK`：能走进火里（并受伤）；`LAVA` flag → `on_enter_cell` 既有岩浆伤害分支直接生效（地形系统零改动）；`LIT`：发光
- `f_info::id::FIRE = 99` 常量 + `tests/f_info_contract.rs` 名单补一行
- 美术零成本：`art.rs` 的 `baseline_material` 见 lava flag → magma material，overlay 可后补

**动态 Glow**：`GlowMask` 目前只在生成/换层时写。引擎每过程 tick 增量维护：FIRE 格点亮（半径 5 置位），熄灭格清除。`compute_fov_map` 每 tick 已读 GlowMask——**视野系统零改动，火光照亮夜路**。实现：引擎持有 `dirty: Vec<idx>` 或全量重算 glow（72k bool，每 8 tick 一次，可接受；选全量重算起步，注释留优化点）。

---

## 7. 守恒复核（rule 7，逐条过）

| 规则 | 复核 |
|---|---|
| fire_spread | FIRE 自我复制，但 FIRE 不可捡拾（不是 Matter），燃烧消耗燃料格 |
| fire_burnout | FIRE→FLOOR/RUBBLE。链条：1 木块(use)→火→烧 N 格→每格 FLOOR/RUBBLE。RUBBLE 可挖=每格 1 块。N 格灰烬 ≤ 烧掉的燃料格数？火蔓延 15% vs 熄灭 6%：期望每火格点燃 ~2.5 邻居前熄灭——**1 木块最多换 ~2.5 灰烬块？** 违规！burnout 产物定为 FLOOR 90/RUBBLE 10（见表），灰烬率约 0.25 块/木块，净负 ✓（概率进 balance.rs，测试断言灰产率 < 1） |
| fire_douse/evaporate | 水灭火：火→FLOOR（燃料格已消耗，无产出）✓；浅水→FLOOR（水格消失，无产出）✓ |
| water_flow_deep | 深水无限产浅水=无限水？浅水可 scoop 成块！**漏洞→已堵**：深水产生 2% ≪ 浅水摊薄 8%，水平衡全局收缩（测试断言水格总数有界） ✓ |
| water_flow_shallow | 邻居+1 浅水、自身→FLOOR：水格数守恒 ✓ |
| grass_spread | 草块无木/铁路径（grass_seed 2 dirt→grass，chop 不吃草），蔓延无经济后果 ✓ |
| 树 | **树不自我复制**（树=2 木等价物，复制即印钞）。树只能 plant（守恒内） |

---

## 8. 事件

```rust
GameEvent::TerrainChanged { at: (i32,i32), from: u16, to: u16, cause: TerrainCause }
GameEvent::Consumed { entity: u64, label: String, hp: i32 }
pub enum TerrainCause { Fire, Water, Growth }
```

`event_visible` 扩展：`TerrainChanged` → at_seen（agent 看见/记得那里才知道变化）；`Consumed` → is_self。viewer/前端事件日志自然接入（formatEvents 补两行）。

---

## 9. 测试（每规则一个行为测试 + 三条铁律）

1. fire_spread：FIRE 邻 TREE → 若干 tick 后 TREE 变 FIRE
2. fire_burnout：FIRE 若干过程 tick 后熄灭（FLOOR 或 RUBBLE）；多次运行统计 RUBBLE 率显著低于 50%
3. fire_douse：FIRE 邻水 → 火灭；浅水概率蒸发
4. 火烧死怪：FIRE 格上 monster_move_to → 死亡（LAVA flag 生效）
5. water_flow：浅水漫延且自身摊薄；深水总量不无限增长（跑 200 过程 tick 断言水格数有界）
6. grass_spread：有水的 DIRT 邻 GRASS → 变 GRASS；无水不变
7. use：木块点燃相邻；草块+1hp；不可用品拒绝
8. 动态 glow：FIRE 格点亮 GlowMask，熄灭清除
9. 确定性：同 seed 两次 process_world 序列结果一致
10. 守恒：完整放火→烧尽循环后，(wood + terrain 块总量) ≤ 初始 + 自然源（测试断言灰产率 < 1）

---

## 10. 文件结构

| 文件 | 责任 |
|---|---|
| `src/process_rules.rs`（新） | 规则表 + CellCond/NeighborCond/ProcessAction/Cause（纯数据） |
| `src/systems/process.rs`（新） | 引擎 process_world + glow 重算 |
| `src/systems/use_item.rs`（新） | use 动词分发 |
| `src/systems/verbs.rs` | 注册表 +1 行 |
| `src/events.rs` | +2 事件变体 + event_visible 两条 |
| `src/tick.rs` | 插入 process_world 阶段 |
| `src/balance.rs` | 速率常量 |
| `src/f_info.rs` + `data/f_info.txt` | FIRE feat + id 常量 |
| `tests/f_info_contract.rs` | 契约名单 +FIRE |
| `tests/sim_rules.rs` | §9 测试 |
| `docs/ARCHITECTURE.md` | tick 阶段图补 process_world；rule 7 守补过程条款 |
