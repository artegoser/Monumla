mod legality;
use monumla_core::{
    Choice, ChoiceKind, ChoiceSpec, EntityId, EntityPresentation, EntityRule, EntityRuleContext,
    EntityState, EntityTypeId, GameOutcome, GameState, History, InteractionError, InteractionFlow,
    InteractionRules, OutcomeRule, PlayerId, PlayerState, Position, PresentationCue, RecordedAction,
    RuleContext, RuleError, RuleRegistry, StateMap, StateValue, TurnRecord, TurnSession,
};
use std::collections::{BTreeSet, VecDeque};

pub const STONE: EntityTypeId = EntityTypeId::new(1);
pub const BLACK: PlayerId = PlayerId::new(1);
pub const WHITE: PlayerId = PlayerId::new(2);

const PHASE: &str = "go.phase";
const PHASE_PLAY: &str = "play";
const PHASE_REVIEW: &str = "review";
const PHASE_FINAL_WHITE_PASS: &str = "final_white_pass";
const PHASE_FINISHED: &str = "finished";
const FINALIZE_MODE: &str = "go.finalize_mode";
const FINALIZE_AGREED: &str = "agreed";
const FINALIZE_ALL_ALIVE: &str = "all_alive";
const CONSECUTIVE_PASSES: &str = "go.consecutive_passes";
const LAST_PASSER: &str = "go.last_passer";
const RESUMED_AFTER_DISPUTE: &str = "go.resumed_after_dispute";
const REVIEW_DONE_BLACK: &str = "go.review_done.black";
const REVIEW_DONE_WHITE: &str = "go.review_done.white";
const REVIEW_DISAGREEMENT: &str = "go.review_disagreement";
const PRISONERS_BLACK: &str = "go.prisoners.black";
const PRISONERS_WHITE: &str = "go.prisoners.white";
const PASS_STONES_BLACK: &str = "go.pass_stones.black";
const PASS_STONES_WHITE: &str = "go.pass_stones.white";
const CONFIG_SCORING: &str = "go.config.scoring";
const CONFIG_KOMI_HALF: &str = "go.config.komi_half";
const CONFIG_HANDICAP: &str = "go.config.handicap";
const RESULT_KIND: &str = "go.result.kind";
const RESULT_WINNER: &str = "go.result.winner";
const RESULT_BLACK_SCORE_HALF: &str = "go.result.black_score_half";
const RESULT_WHITE_SCORE_HALF: &str = "go.result.white_score_half";
const RESULT_BLACK_TERRITORY: &str = "go.result.black_territory";
const RESULT_WHITE_TERRITORY: &str = "go.result.white_territory";
const RESULT_NEUTRAL_POINTS: &str = "go.result.neutral_points";
const DEAD_MARK_BLACK: &str = "go.dead_mark.black";
const DEAD_MARK_WHITE: &str = "go.dead_mark.white";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GoScoring {
    Territory,
    Area,
}

impl GoScoring {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Territory => "territory",
            Self::Area => "area",
        }
    }

    pub fn parse(value: &str) -> Result<Self, RuleError> {
        match value {
            "territory" => Ok(Self::Territory),
            "area" => Ok(Self::Area),
            other => Err(RuleError::Rejected(format!(
                "unknown Go scoring method '{other}'"
            ))),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GoConfig {
    pub size: u16,
    /// Exact komi in half-points. `15` means 7.5 points.
    pub komi_half_points: i64,
    pub scoring: GoScoring,
    /// AGA handicap count. `0` is even, `1` is one-stone handicap (no setup stone),
    /// and `2..=9` use the traditional fixed 19x19 star points.
    pub handicap: u8,
}

impl GoConfig {
    pub const fn standard() -> Self {
        Self {
            size: 19,
            komi_half_points: 15,
            scoring: GoScoring::Territory,
            handicap: 0,
        }
    }

    pub fn aga(size: u16, scoring: GoScoring, handicap: u8) -> Result<Self, RuleError> {
        if size < 2 {
            return Err(RuleError::Rejected(
                "Go board must have at least two lines".into(),
            ));
        }
        if handicap > 9 {
            return Err(RuleError::Rejected(
                "AGA handicap must be between 0 and 9 stones".into(),
            ));
        }
        if handicap >= 2 && size != 19 {
            return Err(RuleError::Rejected(
                "fixed AGA handicaps of 2-9 stones are standardized only on 19x19".into(),
            ));
        }
        Ok(Self {
            size,
            komi_half_points: if handicap == 0 { 15 } else { 1 },
            scoring,
            handicap,
        })
    }
}

impl Default for GoConfig {
    fn default() -> Self {
        Self::standard()
    }
}

pub struct StoneRule;

impl EntityRule for StoneRule {
    fn presentation(
        &self,
        context: EntityRuleContext<'_>,
    ) -> Result<EntityPresentation, RuleError> {
        let side = if context.entity().owner == BLACK {
            "black"
        } else {
            "white"
        };
        let black_mark = entity_bool(context.entity(), DEAD_MARK_BLACK);
        let white_mark = entity_bool(context.entity(), DEAD_MARK_WHITE);
        let mut data = StateMap::new();
        data.insert("dead_mark_black", black_mark);
        data.insert("dead_mark_white", white_mark);
        data.insert("agreed_dead", black_mark && white_mark);
        data.insert("disputed_dead", black_mark != white_mark);
        Ok(EntityPresentation::new(format!("go/{side}/stone"))
            .with_label(format!("{side} stone"))
            .with_data(data))
    }
}

pub fn registry() -> RuleRegistry {
    let mut registry = RuleRegistry::new();
    registry
        .register_entity(STONE, StoneRule)
        .expect("go registers one stone rule");
    registry.register_outcome(GoOutcomeRule);
    registry
}

pub fn state_from_config(config: GoConfig) -> Result<GameState, RuleError> {
    let config = GoConfig::aga(config.size, config.scoring, config.handicap).map(|validated| {
        GoConfig {
            komi_half_points: config.komi_half_points,
            ..validated
        }
    })?;
    let mut state = GameState::new(config.size, config.size)?;
    state.add_player(PlayerState::new(BLACK))?;
    state.add_player(PlayerState::new(WHITE))?;
    initialize_ruleset_state(&mut state, config);

    if config.handicap >= 2 {
        for (index, position) in handicap_positions(config.handicap)?.into_iter().enumerate() {
            state.add_entity(EntityState::new(
                EntityId::new(u32::try_from(index + 1).expect("at most nine handicap stones")),
                STONE,
                BLACK,
                position,
            ))?;
        }
        state.set_active_players(vec![WHITE])?;
    } else {
        state.set_active_players(vec![BLACK])?;
    }
    Ok(state)
}

/// Backwards-compatible helper used by tests and embedders. It creates an even
/// AGA game with 7.5 komi and territory scoring on the requested board size.
pub fn empty_state(size: u16) -> GameState {
    state_from_config(GoConfig::aga(size, GoScoring::Territory, 0).expect("valid Go size"))
        .expect("valid Go state")
}

fn initialize_ruleset_state(state: &mut GameState, config: GoConfig) {
    state.ruleset_state.insert(PHASE, PHASE_PLAY);
    state.ruleset_state.insert(CONSECUTIVE_PASSES, 0_u64);
    state.ruleset_state.insert(RESUMED_AFTER_DISPUTE, false);
    state.ruleset_state.insert(REVIEW_DONE_BLACK, false);
    state.ruleset_state.insert(REVIEW_DONE_WHITE, false);
    state.ruleset_state.insert(REVIEW_DISAGREEMENT, false);
    state.ruleset_state.insert(PRISONERS_BLACK, 0_u64);
    state.ruleset_state.insert(PRISONERS_WHITE, 0_u64);
    state.ruleset_state.insert(PASS_STONES_BLACK, 0_u64);
    state.ruleset_state.insert(PASS_STONES_WHITE, 0_u64);
    state
        .ruleset_state
        .insert(CONFIG_SCORING, config.scoring.as_str());
    state
        .ruleset_state
        .insert(CONFIG_KOMI_HALF, config.komi_half_points);
    state
        .ruleset_state
        .insert(CONFIG_HANDICAP, u64::from(config.handicap));
}

fn handicap_positions(handicap: u8) -> Result<Vec<Position>, RuleError> {
    let q16 = Position::new(15, 15);
    let d4 = Position::new(3, 3);
    let q4 = Position::new(15, 3);
    let d16 = Position::new(3, 15);
    let q10 = Position::new(15, 9);
    let d10 = Position::new(3, 9);
    let k16 = Position::new(9, 15);
    let k4 = Position::new(9, 3);
    let k10 = Position::new(9, 9);
    let sequence = [q16, d4, q4, d16, q10, d10, k16, k4];
    let result = match handicap {
        0 | 1 => Vec::new(),
        2..=4 => sequence[..usize::from(handicap)].to_vec(),
        5 => vec![q16, d4, q4, d16, k10],
        6 => sequence[..6].to_vec(),
        7 => vec![q16, d4, q4, d16, q10, d10, k10],
        8 => sequence.to_vec(),
        9 => vec![q16, d4, q4, d16, q10, d10, k16, k4, k10],
        _ => {
            return Err(RuleError::Rejected(
                "AGA handicap must be between 0 and 9 stones".into(),
            ))
        }
    };
    Ok(result)
}

fn active_player(state: &GameState) -> Result<PlayerId, InteractionError> {
    let [player] = state.turn.active_players.as_slice() else {
        return Err(InteractionError::RuleViolation(
            "Go requires exactly one active player while the game is live".into(),
        ));
    };
    Ok(*player)
}

fn opponent(player: PlayerId) -> Result<PlayerId, InteractionError> {
    match player {
        BLACK => Ok(WHITE),
        WHITE => Ok(BLACK),
        _ => Err(InteractionError::RuleViolation("unknown Go player".into())),
    }
}

fn opponent_rule(player: PlayerId) -> Result<PlayerId, RuleError> {
    match player {
        BLACK => Ok(WHITE),
        WHITE => Ok(BLACK),
        _ => Err(RuleError::Rejected("unknown Go player".into())),
    }
}

fn player_name(player: PlayerId) -> &'static str {
    if player == BLACK {
        "Black"
    } else if player == WHITE {
        "White"
    } else {
        "Unknown"
    }
}

fn phase(state: &GameState) -> &str {
    state
        .ruleset_state
        .get(PHASE)
        .and_then(StateValue::as_str)
        .unwrap_or(PHASE_PLAY)
}

fn state_u64(state: &GameState, key: &str) -> u64 {
    state
        .ruleset_state
        .get(key)
        .and_then(StateValue::as_u64)
        .unwrap_or(0)
}

fn state_i64(state: &GameState, key: &str) -> i64 {
    state
        .ruleset_state
        .get(key)
        .and_then(StateValue::as_i64)
        .unwrap_or(0)
}

fn state_bool(state: &GameState, key: &str) -> bool {
    state
        .ruleset_state
        .get(key)
        .and_then(StateValue::as_bool)
        .unwrap_or(false)
}

fn entity_bool(entity: &EntityState, key: &str) -> bool {
    entity.state.get(key).and_then(StateValue::as_bool).unwrap_or(false)
}

fn mark_key(player: PlayerId) -> Result<&'static str, InteractionError> {
    match player {
        BLACK => Ok(DEAD_MARK_BLACK),
        WHITE => Ok(DEAD_MARK_WHITE),
        _ => Err(InteractionError::RuleViolation("unknown Go player".into())),
    }
}

fn review_done_key(player: PlayerId) -> Result<&'static str, InteractionError> {
    match player {
        BLACK => Ok(REVIEW_DONE_BLACK),
        WHITE => Ok(REVIEW_DONE_WHITE),
        _ => Err(InteractionError::RuleViolation("unknown Go player".into())),
    }
}

fn prisoner_key(player: PlayerId) -> Result<&'static str, InteractionError> {
    match player {
        BLACK => Ok(PRISONERS_BLACK),
        WHITE => Ok(PRISONERS_WHITE),
        _ => Err(InteractionError::RuleViolation("unknown Go player".into())),
    }
}

fn pass_stone_key(player: PlayerId) -> Result<&'static str, InteractionError> {
    match player {
        BLACK => Ok(PASS_STONES_BLACK),
        WHITE => Ok(PASS_STONES_WHITE),
        _ => Err(InteractionError::RuleViolation("unknown Go player".into())),
    }
}

fn add_counter(state: &mut GameState, key: &str, amount: u64) {
    let current = state
        .ruleset_state
        .get(key)
        .and_then(StateValue::as_u64)
        .unwrap_or(0);
    state
        .ruleset_state
        .insert(key, current.saturating_add(amount));
}

fn give_pass_stone(state: &mut GameState, passer: PlayerId) -> Result<(), InteractionError> {
    let receiver = opponent(passer)?;
    add_counter(state, prisoner_key(receiver)?, 1);
    add_counter(state, pass_stone_key(receiver)?, 1);
    state
        .ruleset_state
        .insert(LAST_PASSER, u64::from(passer.get()));
    Ok(())
}

fn neighbors(state: &GameState, position: Position) -> Vec<Position> {
    let mut result = Vec::with_capacity(4);
    if position.x > 0 {
        result.push(Position::new(position.x - 1, position.y));
    }
    if position.x + 1 < state.board.width() {
        result.push(Position::new(position.x + 1, position.y));
    }
    if position.y > 0 {
        result.push(Position::new(position.x, position.y - 1));
    }
    if position.y + 1 < state.board.height() {
        result.push(Position::new(position.x, position.y + 1));
    }
    result
}

fn group(state: &GameState, start: EntityId) -> Result<BTreeSet<EntityId>, InteractionError> {
    let owner = state.entity(start)?.owner;
    let mut seen = BTreeSet::new();
    let mut queue = VecDeque::from([start]);
    while let Some(entity) = queue.pop_front() {
        if !seen.insert(entity) {
            continue;
        }
        let position = state.entity(entity)?.position;
        for neighbor in neighbors(state, position) {
            let Some(other) = state.entity_at(neighbor)? else {
                continue;
            };
            if other.entity_type == STONE && other.owner == owner && !seen.contains(&other.id) {
                queue.push_back(other.id);
            }
        }
    }
    Ok(seen)
}

#[cfg(test)]
fn group_rule(state: &GameState, start: EntityId) -> Result<BTreeSet<EntityId>, RuleError> {
    let owner = state.entity(start)?.owner;
    let mut seen = BTreeSet::new();
    let mut queue = VecDeque::from([start]);
    while let Some(entity) = queue.pop_front() {
        if !seen.insert(entity) {
            continue;
        }
        let position = state.entity(entity)?.position;
        for neighbor in neighbors(state, position) {
            let Some(other) = state.entity_at(neighbor)? else {
                continue;
            };
            if other.entity_type == STONE && other.owner == owner && !seen.contains(&other.id) {
                queue.push_back(other.id);
            }
        }
    }
    Ok(seen)
}

fn group_has_liberty(
    state: &GameState,
    stones: &BTreeSet<EntityId>,
) -> Result<bool, InteractionError> {
    for entity in stones {
        let position = state.entity(*entity)?.position;
        for neighbor in neighbors(state, position) {
            if state.entity_at(neighbor)?.is_none() {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

fn next_entity_id(state: &GameState) -> Result<EntityId, InteractionError> {
    let max = state.entities.keys().map(|id| id.get()).max().unwrap_or(0);
    let next = max.checked_add(1).ok_or_else(|| {
        InteractionError::RuleViolation("Go stone entity id space exhausted".into())
    })?;
    Ok(EntityId::new(next))
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct BoardKey(Vec<u8>);

fn board_key(state: &GameState) -> Result<BoardKey, InteractionError> {
    let mut cells = Vec::with_capacity(
        usize::from(state.board.width()).saturating_mul(usize::from(state.board.height())),
    );
    for position in state.board.positions() {
        let value = match state.entity_at(position)? {
            None => 0,
            Some(entity) if entity.owner == BLACK => 1,
            Some(entity) if entity.owner == WHITE => 2,
            Some(_) => {
                return Err(InteractionError::RuleViolation(
                    "Go board contains an entity owned by an unknown player".into(),
                ))
            }
        };
        cells.push(value);
    }
    Ok(BoardKey(cells))
}

#[cfg(test)]
fn board_key_rule(state: &GameState) -> Result<BoardKey, RuleError> {
    let mut cells = Vec::with_capacity(
        usize::from(state.board.width()).saturating_mul(usize::from(state.board.height())),
    );
    for position in state.board.positions() {
        let value = match state.entity_at(position)? {
            None => 0,
            Some(entity) if entity.owner == BLACK => 1,
            Some(entity) if entity.owner == WHITE => 2,
            Some(_) => return Err(RuleError::Rejected("invalid Go stone owner".into())),
        };
        cells.push(value);
    }
    Ok(BoardKey(cells))
}

fn historical_play_positions(
    history: Option<&History>,
) -> Result<BTreeSet<(PlayerId, BoardKey)>, InteractionError> {
    let mut seen = BTreeSet::new();
    let Some(history) = history else {
        return Ok(seen);
    };

    // Situational superko compares full-board stones plus the player to move.
    // Pass itself is always legal, but the play-phase situation after a pass is
    // still a previously seen situation that a later placement may not recreate.
    if let Some(first) = history.turns().first() {
        if phase(&first.before) == PHASE_PLAY {
            if let [player] = first.before.turn.active_players.as_slice() {
                seen.insert((*player, board_key(&first.before)?));
            }
        }
    }
    for turn in history.turns() {
        if phase(&turn.after) != PHASE_PLAY {
            continue;
        }
        let [next] = turn.after.turn.active_players.as_slice() else {
            continue;
        };
        seen.insert((*next, board_key(&turn.after)?));
    }
    Ok(seen)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct PlacementResult {
    stone: EntityId,
    captured: u64,
}

fn apply_placement(
    state: &mut GameState,
    player: PlayerId,
    position: Position,
) -> Result<PlacementResult, InteractionError> {
    if phase(state) != PHASE_PLAY {
        return Err(InteractionError::RuleViolation(
            "stones may only be played during the Go play phase".into(),
        ));
    }
    if state.entity_at(position)?.is_some() {
        return Err(InteractionError::RuleViolation(
            "Go point is already occupied".into(),
        ));
    }
    let stone = next_entity_id(state)?;
    state.add_entity(EntityState::new(stone, STONE, player, position))?;

    let enemy = opponent(player)?;
    let adjacent_enemy = neighbors(state, position)
        .into_iter()
        .filter_map(|neighbor| state.entity_at(neighbor).ok().flatten())
        .filter(|entity| entity.owner == enemy)
        .map(|entity| entity.id)
        .collect::<BTreeSet<_>>();

    let mut captured = BTreeSet::new();
    for enemy_stone in adjacent_enemy {
        if !state.entities.contains_key(&enemy_stone) || captured.contains(&enemy_stone) {
            continue;
        }
        let enemy_group = group(state, enemy_stone)?;
        if !group_has_liberty(state, &enemy_group)? {
            captured.extend(enemy_group);
        }
    }
    let captured_count = u64::try_from(captured.len()).unwrap_or(u64::MAX);
    for entity in captured {
        state.remove_entity(entity)?;
    }

    let own_group = group(state, stone)?;
    if !group_has_liberty(state, &own_group)? {
        return Err(InteractionError::RuleViolation(
            "self-capture is illegal under AGA rules".into(),
        ));
    }
    Ok(PlacementResult {
        stone,
        captured: captured_count,
    })
}

#[cfg(test)]
fn placement_is_legal(
    state: &GameState,
    seen: &BTreeSet<(PlayerId, BoardKey)>,
    player: PlayerId,
    position: Position,
) -> Result<bool, InteractionError> {
    if phase(state) != PHASE_PLAY || state.entity_at(position)?.is_some() {
        return Ok(false);
    }
    let mut candidate = state.clone();
    if apply_placement(&mut candidate, player, position).is_err() {
        return Ok(false);
    }
    let next = opponent(player)?;
    if seen.contains(&(next, board_key(&candidate)?)) {
        return Ok(false);
    }
    Ok(true)
}

fn clear_review_marks(state: &mut GameState) {
    for entity in state.entities.values_mut() {
        entity.state.remove(DEAD_MARK_BLACK);
        entity.state.remove(DEAD_MARK_WHITE);
    }
    state.ruleset_state.insert(REVIEW_DONE_BLACK, false);
    state.ruleset_state.insert(REVIEW_DONE_WHITE, false);
    state.ruleset_state.insert(REVIEW_DISAGREEMENT, false);
}

fn review_sets_equal(state: &GameState) -> bool {
    state.entities.values().all(|entity| {
        entity_bool(entity, DEAD_MARK_BLACK) == entity_bool(entity, DEAD_MARK_WHITE)
    })
}

fn toggle_group_mark(
    state: &mut GameState,
    actor: PlayerId,
    entity: EntityId,
) -> Result<(), InteractionError> {
    if phase(state) != PHASE_REVIEW {
        return Err(InteractionError::RuleViolation(
            "dead groups can only be marked during scoring review".into(),
        ));
    }
    let stones = group(state, entity)?;
    let key = mark_key(actor)?;
    let marked = stones
        .iter()
        .next()
        .and_then(|id| state.entities.get(id))
        .map(|stone| entity_bool(stone, key))
        .unwrap_or(false);
    for stone in stones {
        state.entity_mut(stone)?.state.insert(key, !marked);
    }
    state.ruleset_state.insert(review_done_key(actor)?, false);
    state.ruleset_state.insert(REVIEW_DISAGREEMENT, false);
    Ok(())
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct TerritoryCount {
    black: u64,
    white: u64,
    neutral: u64,
}

fn territory(state: &GameState) -> Result<TerritoryCount, RuleError> {
    let mut visited = BTreeSet::new();
    let mut result = TerritoryCount::default();

    for start in state.board.positions() {
        if visited.contains(&start) || state.entity_at(start)?.is_some() {
            continue;
        }
        let mut region = BTreeSet::new();
        let mut border = BTreeSet::new();
        let mut queue = VecDeque::from([start]);
        while let Some(position) = queue.pop_front() {
            if !region.insert(position) {
                continue;
            }
            for neighbor in neighbors(state, position) {
                if let Some(entity) = state.entity_at(neighbor)? {
                    border.insert(entity.owner);
                } else if !region.contains(&neighbor) {
                    queue.push_back(neighbor);
                }
            }
        }
        visited.extend(region.iter().copied());
        let points = u64::try_from(region.len()).unwrap_or(u64::MAX);
        if border.len() == 1 {
            let owner = *border.iter().next().expect("one border owner");
            if owner == BLACK {
                result.black = result.black.saturating_add(points);
            } else if owner == WHITE {
                result.white = result.white.saturating_add(points);
            } else {
                result.neutral = result.neutral.saturating_add(points);
            }
        } else {
            result.neutral = result.neutral.saturating_add(points);
        }
    }
    Ok(result)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Score {
    black_half: i64,
    white_half: i64,
    territory: TerritoryCount,
}

fn scoring_method(state: &GameState) -> Result<GoScoring, RuleError> {
    let value = state
        .ruleset_state
        .get(CONFIG_SCORING)
        .and_then(StateValue::as_str)
        .unwrap_or("territory");
    GoScoring::parse(value)
}

fn score_position(state: &GameState) -> Result<Score, RuleError> {
    let territory = territory(state)?;
    let black_live = u64::try_from(
        state
            .entities
            .values()
            .filter(|entity| entity.entity_type == STONE && entity.owner == BLACK)
            .count(),
    )
    .unwrap_or(u64::MAX);
    let white_live = u64::try_from(
        state
            .entities
            .values()
            .filter(|entity| entity.entity_type == STONE && entity.owner == WHITE)
            .count(),
    )
    .unwrap_or(u64::MAX);
    let komi_half = state_i64(state, CONFIG_KOMI_HALF);
    let handicap = state_u64(state, CONFIG_HANDICAP);

    let (black_half, white_half) = match scoring_method(state)? {
        GoScoring::Territory => {
            // AGA territory counting scores each side's surrounded territory minus
            // the prisoners held by the opponent. Pass stones and agreed dead
            // stones are ordinary prisoners, so they naturally enter here.
            let black_lost = state_u64(state, PRISONERS_WHITE);
            let white_lost = state_u64(state, PRISONERS_BLACK);
            (
                half_points_signed(territory.black, black_lost),
                half_points_signed(territory.white, white_lost).saturating_add(komi_half),
            )
        }
        GoScoring::Area => {
            let handicap_compensation = handicap.saturating_sub(1);
            (
                half_points(territory.black.saturating_add(black_live)),
                half_points(territory.white.saturating_add(white_live))
                    .saturating_add(komi_half)
                    .saturating_add(half_points(handicap_compensation)),
            )
        }
    };

    Ok(Score {
        black_half,
        white_half,
        territory,
    })
}

fn half_points_signed(positive: u64, negative: u64) -> i64 {
    let positive = i64::try_from(positive).unwrap_or(i64::MAX / 2);
    let negative = i64::try_from(negative).unwrap_or(i64::MAX / 2);
    positive.saturating_sub(negative).saturating_mul(2)
}

fn half_points(points: u64) -> i64 {
    i64::try_from(points)
        .unwrap_or(i64::MAX / 2)
        .saturating_mul(2)
}

fn remove_agreed_dead(state: &mut GameState) -> Result<(), InteractionError> {
    let dead = state
        .entities
        .values()
        .filter(|entity| entity_bool(entity, DEAD_MARK_BLACK) && entity_bool(entity, DEAD_MARK_WHITE))
        .map(|entity| entity.id)
        .collect::<Vec<_>>();
    for entity in dead {
        let owner = state.entity(entity)?.owner;
        let captor = opponent(owner)?;
        add_counter(state, prisoner_key(captor)?, 1);
        state.remove_entity(entity)?;
    }
    Ok(())
}

fn finish_scoring(state: &mut GameState, mode: &str) -> Result<(), InteractionError> {
    if mode == FINALIZE_AGREED {
        remove_agreed_dead(state)?;
    } else if mode != FINALIZE_ALL_ALIVE {
        return Err(InteractionError::RuleViolation(
            "unknown Go scoring finalization mode".into(),
        ));
    }
    clear_review_marks(state);
    let score = score_position(state)
        .map_err(|error| InteractionError::RuleViolation(error.to_string()))?;
    state.ruleset_state.insert(PHASE, PHASE_FINISHED);
    state.ruleset_state.insert(RESULT_KIND, "score");
    state
        .ruleset_state
        .insert(RESULT_BLACK_SCORE_HALF, score.black_half);
    state
        .ruleset_state
        .insert(RESULT_WHITE_SCORE_HALF, score.white_half);
    state
        .ruleset_state
        .insert(RESULT_BLACK_TERRITORY, score.territory.black);
    state
        .ruleset_state
        .insert(RESULT_WHITE_TERRITORY, score.territory.white);
    state
        .ruleset_state
        .insert(RESULT_NEUTRAL_POINTS, score.territory.neutral);
    if score.black_half > score.white_half {
        state
            .ruleset_state
            .insert(RESULT_WINNER, u64::from(BLACK.get()));
    } else if score.white_half > score.black_half {
        state
            .ruleset_state
            .insert(RESULT_WINNER, u64::from(WHITE.get()));
    } else {
        state.ruleset_state.remove(RESULT_WINNER);
    }
    state.set_active_players(Vec::new())?;
    Ok(())
}

fn enter_review(state: &mut GameState, last_passer: PlayerId) -> Result<(), InteractionError> {
    state.ruleset_state.insert(PHASE, PHASE_REVIEW);
    state
        .ruleset_state
        .insert(LAST_PASSER, u64::from(last_passer.get()));
    state.ruleset_state.insert(REVIEW_DONE_BLACK, false);
    state.ruleset_state.insert(REVIEW_DONE_WHITE, false);
    state.ruleset_state.insert(REVIEW_DISAGREEMENT, false);
    state.set_active_players(vec![opponent(last_passer)?])?;
    Ok(())
}

fn begin_final_white_pass(
    state: &mut GameState,
    finalize_mode: &str,
) -> Result<(), InteractionError> {
    state.ruleset_state.insert(PHASE, PHASE_FINAL_WHITE_PASS);
    state.ruleset_state.insert(FINALIZE_MODE, finalize_mode);
    state.set_active_players(vec![WHITE])?;
    Ok(())
}

fn finish_or_require_white_last(
    state: &mut GameState,
    finalize_mode: &str,
) -> Result<(), InteractionError> {
    let last_passer = state_u64(state, LAST_PASSER);
    if last_passer == u64::from(BLACK.get()) {
        begin_final_white_pass(state, finalize_mode)
    } else {
        finish_scoring(state, finalize_mode)
    }
}

fn record_resignation(state: &mut GameState, loser: PlayerId) -> Result<(), InteractionError> {
    let winner = opponent(loser)?;
    state.ruleset_state.insert(PHASE, PHASE_FINISHED);
    state.ruleset_state.insert(RESULT_KIND, "resignation");
    state
        .ruleset_state
        .insert(RESULT_WINNER, u64::from(winner.get()));
    state.ruleset_state.remove(RESULT_BLACK_SCORE_HALF);
    state.ruleset_state.remove(RESULT_WHITE_SCORE_HALF);
    state.ruleset_state.remove(RESULT_BLACK_TERRITORY);
    state.ruleset_state.remove(RESULT_WHITE_TERRITORY);
    state.ruleset_state.remove(RESULT_NEUTRAL_POINTS);
    state.set_active_players(Vec::new())?;
    Ok(())
}

#[derive(Clone, Default)]
pub struct GoInteractionRules {
    seen_positions: BTreeSet<(PlayerId, BoardKey)>,
}

impl GoInteractionRules {
    pub fn with_history(history: &History) -> Result<Self, InteractionError> {
        Ok(Self {
            seen_positions: historical_play_positions(Some(history))?,
        })
    }

    fn play_choices(
        &self,
        state: &GameState,
        player: PlayerId,
    ) -> Result<Vec<ChoiceSpec>, InteractionError> {
        let mut choices = Vec::new();
        let facts = legality::PlacementFacts::new(state)?;
        for position in state.board.positions() {
            if facts.legal(state, &self.seen_positions, player, position)? {
                choices.push(ChoiceSpec::position(position));
            }
        }
        choices.push(ChoiceSpec::option("pass").with_label("Pass"));
        choices.push(ChoiceSpec::option("resign").with_label("Resign"));
        Ok(choices)
    }

    fn review_choices(
        &self,
        state: &GameState,
    ) -> Result<Vec<ChoiceSpec>, InteractionError> {
        let mut choices = state
            .entities
            .values()
            .filter(|entity| entity.entity_type == STONE)
            .map(|entity| ChoiceSpec::entity(entity.id).with_label("Toggle dead group"))
            .collect::<Vec<_>>();
        choices.push(ChoiceSpec::option("review_done").with_label("Done reviewing"));
        choices.push(ChoiceSpec::option("resume_play").with_label("Resume play"));
        choices.push(ChoiceSpec::option("resign").with_label("Resign"));
        Ok(choices)
    }
}

impl InteractionRules for GoInteractionRules {
    fn choices(
        &self,
        turn: &TurnSession,
        _draft: &StateMap,
    ) -> Result<Vec<ChoiceSpec>, InteractionError> {
        let player = active_player(&turn.working)?;
        match phase(&turn.working) {
            PHASE_PLAY => self.play_choices(&turn.working, player),
            PHASE_REVIEW => self.review_choices(&turn.working),
            PHASE_FINAL_WHITE_PASS => {
                let mut choices = vec![ChoiceSpec::option("final_pass").with_label("White final pass")];
                choices.push(ChoiceSpec::option("resign").with_label("Resign"));
                Ok(choices)
            }
            PHASE_FINISHED => Ok(Vec::new()),
            other => Err(InteractionError::RuleViolation(format!(
                "unknown Go phase '{other}'"
            ))),
        }
    }

    fn apply_choice(
        &self,
        turn: &mut TurnSession,
        _draft: &mut StateMap,
        choice: &Choice,
    ) -> Result<InteractionFlow, InteractionError> {
        let player = active_player(&turn.working)?;
        match phase(&turn.working) {
            PHASE_PLAY => self.apply_play_choice(turn, player, choice),
            PHASE_REVIEW => self.apply_review_choice(turn, player, choice),
            PHASE_FINAL_WHITE_PASS => self.apply_final_pass_choice(turn, player, choice),
            PHASE_FINISHED => Err(InteractionError::RuleViolation(
                "the Go game has already finished".into(),
            )),
            other => Err(InteractionError::RuleViolation(format!(
                "unknown Go phase '{other}'"
            ))),
        }
    }
}

impl GoInteractionRules {
    fn apply_play_choice(
        &self,
        turn: &mut TurnSession,
        player: PlayerId,
        choice: &Choice,
    ) -> Result<InteractionFlow, InteractionError> {
        match &choice.kind {
            ChoiceKind::SelectPosition { position } => {
                let mut action = RecordedAction::new("go.place");
                action.data.insert("x", u64::from(position.x));
                action.data.insert("y", u64::from(position.y));
                turn.apply_transaction(
                    action,
                    |transaction| -> Result<(), InteractionError> {
                        let result = apply_placement(transaction.raw_state_mut(), player, *position)?;
                        if self
                            .seen_positions
                            .contains(&(opponent(player)?, board_key(transaction.state())?))
                        {
                            return Err(InteractionError::RuleViolation(
                                "AGA situational superko forbids this placement".into(),
                            ));
                        }
                        if result.captured > 0 {
                            add_counter(
                                transaction.raw_state_mut(),
                                prisoner_key(player)?,
                                result.captured,
                            );
                        }
                        transaction
                            .ruleset_state_mut()
                            .insert(CONSECUTIVE_PASSES, 0_u64);
                        transaction
                            .ruleset_state_mut()
                            .insert(RESUMED_AFTER_DISPUTE, false);
                        transaction
                            .raw_state_mut()
                            .set_active_players(vec![opponent(player)?])?;
                        let mut data = StateMap::new();
                        data.insert("stone", u64::from(result.stone.get()));
                        data.insert("x", u64::from(position.x));
                        data.insert("y", u64::from(position.y));
                        data.insert("captured", result.captured);
                        transaction.present(PresentationCue::new("go.place").with_data(data));
                        Ok(())
                    },
                )?;
                Ok(InteractionFlow::FinishTurn)
            }
            ChoiceKind::SelectOption { key } if key == "pass" => {
                let current_passes = state_u64(&turn.working, CONSECUTIVE_PASSES);
                let resumed = state_bool(&turn.working, RESUMED_AFTER_DISPUTE);
                turn.apply_transaction(
                    RecordedAction::new("go.pass"),
                    |transaction| -> Result<(), InteractionError> {
                        give_pass_stone(transaction.raw_state_mut(), player)?;
                        let next_passes = current_passes.saturating_add(1);
                        transaction
                            .ruleset_state_mut()
                            .insert(CONSECUTIVE_PASSES, next_passes);
                        if next_passes >= 2 {
                            if resumed {
                                finish_or_require_white_last(
                                    transaction.raw_state_mut(),
                                    FINALIZE_ALL_ALIVE,
                                )?;
                            } else {
                                enter_review(transaction.raw_state_mut(), player)?;
                            }
                        } else {
                            transaction
                                .raw_state_mut()
                                .set_active_players(vec![opponent(player)?])?;
                        }
                        Ok(())
                    },
                )?;
                Ok(InteractionFlow::FinishTurn)
            }
            ChoiceKind::SelectOption { key } if key == "resign" => {
                turn.apply_transaction(
                    RecordedAction::new("go.resign"),
                    |transaction| -> Result<(), InteractionError> {
                        record_resignation(transaction.raw_state_mut(), player)
                    },
                )?;
                Ok(InteractionFlow::FinishTurn)
            }
            _ => Err(InteractionError::RuleViolation(
                "unexpected Go play choice".into(),
            )),
        }
    }

    fn apply_review_choice(
        &self,
        turn: &mut TurnSession,
        player: PlayerId,
        choice: &Choice,
    ) -> Result<InteractionFlow, InteractionError> {
        match &choice.kind {
            ChoiceKind::SelectEntity { entity } => {
                if turn.working.entity(*entity)?.entity_type != STONE {
                    return Err(InteractionError::RuleViolation(
                        "only Go stone groups can be reviewed".into(),
                    ));
                }
                let mut action = RecordedAction::new("go.mark_dead");
                action.data.insert("entity", u64::from(entity.get()));
                turn.apply_transaction(
                    action,
                    |transaction| -> Result<(), InteractionError> {
                        toggle_group_mark(transaction.raw_state_mut(), player, *entity)
                    },
                )?;
                Ok(InteractionFlow::FinishTurn)
            }
            ChoiceKind::SelectOption { key } if key == "review_done" => {
                let own_done_key = review_done_key(player)?;
                let other = opponent(player)?;
                let other_done_key = review_done_key(other)?;
                turn.apply_transaction(
                    RecordedAction::new("go.review_done"),
                    |transaction| -> Result<(), InteractionError> {
                        transaction.ruleset_state_mut().insert(own_done_key, true);
                        let other_done = transaction
                            .state()
                            .ruleset_state
                            .get(other_done_key)
                            .and_then(StateValue::as_bool)
                            .unwrap_or(false);
                        if !other_done {
                            transaction.raw_state_mut().set_active_players(vec![other])?;
                            return Ok(());
                        }
                        if review_sets_equal(transaction.state()) {
                            finish_or_require_white_last(
                                transaction.raw_state_mut(),
                                FINALIZE_AGREED,
                            )?;
                        } else {
                            transaction
                                .ruleset_state_mut()
                                .insert(REVIEW_DISAGREEMENT, true);
                            transaction
                                .ruleset_state_mut()
                                .insert(REVIEW_DONE_BLACK, false);
                            transaction
                                .ruleset_state_mut()
                                .insert(REVIEW_DONE_WHITE, false);
                            let last = PlayerId::new(
                                u32::try_from(state_u64(transaction.state(), LAST_PASSER))
                                    .unwrap_or(WHITE.get()),
                            );
                            transaction
                                .raw_state_mut()
                                .set_active_players(vec![opponent(last)?])?;
                        }
                        Ok(())
                    },
                )?;
                Ok(InteractionFlow::FinishTurn)
            }
            ChoiceKind::SelectOption { key } if key == "resume_play" => {
                let last = PlayerId::new(
                    u32::try_from(state_u64(&turn.working, LAST_PASSER)).unwrap_or(WHITE.get()),
                );
                turn.apply_transaction(
                    RecordedAction::new("go.resume"),
                    |transaction| -> Result<(), InteractionError> {
                        clear_review_marks(transaction.raw_state_mut());
                        transaction.ruleset_state_mut().insert(PHASE, PHASE_PLAY);
                        transaction
                            .ruleset_state_mut()
                            .insert(CONSECUTIVE_PASSES, 0_u64);
                        transaction
                            .ruleset_state_mut()
                            .insert(RESUMED_AFTER_DISPUTE, true);
                        transaction
                            .raw_state_mut()
                            .set_active_players(vec![opponent(last)?])?;
                        Ok(())
                    },
                )?;
                Ok(InteractionFlow::FinishTurn)
            }
            ChoiceKind::SelectOption { key } if key == "resign" => {
                turn.apply_transaction(
                    RecordedAction::new("go.resign"),
                    |transaction| -> Result<(), InteractionError> {
                        record_resignation(transaction.raw_state_mut(), player)
                    },
                )?;
                Ok(InteractionFlow::FinishTurn)
            }
            _ => Err(InteractionError::RuleViolation(
                "unexpected Go scoring-review choice".into(),
            )),
        }
    }

    fn apply_final_pass_choice(
        &self,
        turn: &mut TurnSession,
        player: PlayerId,
        choice: &Choice,
    ) -> Result<InteractionFlow, InteractionError> {
        if player != WHITE {
            return Err(InteractionError::RuleViolation(
                "AGA final pass must be made by White".into(),
            ));
        }
        match &choice.kind {
            ChoiceKind::SelectOption { key } if key == "final_pass" => {
                let mode = turn
                    .working
                    .ruleset_state
                    .get(FINALIZE_MODE)
                    .and_then(StateValue::as_str)
                    .unwrap_or(FINALIZE_AGREED)
                    .to_owned();
                turn.apply_transaction(
                    RecordedAction::new("go.final_pass"),
                    |transaction| -> Result<(), InteractionError> {
                        give_pass_stone(transaction.raw_state_mut(), WHITE)?;
                        finish_scoring(transaction.raw_state_mut(), &mode)
                    },
                )?;
                Ok(InteractionFlow::FinishTurn)
            }
            ChoiceKind::SelectOption { key } if key == "resign" => {
                turn.apply_transaction(
                    RecordedAction::new("go.resign"),
                    |transaction| -> Result<(), InteractionError> {
                        record_resignation(transaction.raw_state_mut(), WHITE)
                    },
                )?;
                Ok(InteractionFlow::FinishTurn)
            }
            _ => Err(InteractionError::RuleViolation(
                "unexpected Go final-pass choice".into(),
            )),
        }
    }
}

pub struct GoOutcomeRule;

impl OutcomeRule for GoOutcomeRule {
    fn evaluate(&self, context: RuleContext<'_>) -> Result<Option<GameOutcome>, RuleError> {
        let state = context.state();
        let Some(kind) = state
            .ruleset_state
            .get(RESULT_KIND)
            .and_then(StateValue::as_str)
        else {
            return Ok(None);
        };
        let winner = state
            .ruleset_state
            .get(RESULT_WINNER)
            .and_then(StateValue::as_u64)
            .and_then(|value| u32::try_from(value).ok())
            .map(PlayerId::new);
        let mut data = status_details(state);
        data.insert("result_kind", kind);
        let mut outcome = GameOutcome::new(format!("go.{kind}")).with_data(data);
        if let Some(winner) = winner {
            outcome = outcome
                .with_winner(winner)
                .with_loser(opponent_rule(winner)?);
        }
        Ok(Some(outcome))
    }
}

pub fn status_details(state: &GameState) -> StateMap {
    let mut details = StateMap::new();
    details.insert("phase", phase(state));
    details.insert(
        "scoring",
        state
            .ruleset_state
            .get(CONFIG_SCORING)
            .and_then(StateValue::as_str)
            .unwrap_or("territory"),
    );
    details.insert("komi_half", state_i64(state, CONFIG_KOMI_HALF));
    details.insert("handicap", state_u64(state, CONFIG_HANDICAP));
    details.insert("black_prisoners", state_u64(state, PRISONERS_BLACK));
    details.insert("white_prisoners", state_u64(state, PRISONERS_WHITE));
    details.insert(
        "black_pass_stones",
        state_u64(state, PASS_STONES_BLACK),
    );
    details.insert(
        "white_pass_stones",
        state_u64(state, PASS_STONES_WHITE),
    );
    details.insert(
        "review_disagreement",
        state_bool(state, REVIEW_DISAGREEMENT),
    );
    if let Some(score) = state
        .ruleset_state
        .get(RESULT_BLACK_SCORE_HALF)
        .and_then(StateValue::as_i64)
    {
        details.insert("black_score_half", score);
    }
    if let Some(score) = state
        .ruleset_state
        .get(RESULT_WHITE_SCORE_HALF)
        .and_then(StateValue::as_i64)
    {
        details.insert("white_score_half", score);
    }
    if let Some(points) = state
        .ruleset_state
        .get(RESULT_BLACK_TERRITORY)
        .and_then(StateValue::as_u64)
    {
        details.insert("black_territory", points);
    }
    if let Some(points) = state
        .ruleset_state
        .get(RESULT_WHITE_TERRITORY)
        .and_then(StateValue::as_u64)
    {
        details.insert("white_territory", points);
    }
    if let Some(points) = state
        .ruleset_state
        .get(RESULT_NEUTRAL_POINTS)
        .and_then(StateValue::as_u64)
    {
        details.insert("neutral_points", points);
    }
    details
}

pub fn status_text(state: &GameState) -> String {
    if phase(state) == PHASE_FINISHED {
        if state
            .ruleset_state
            .get(RESULT_KIND)
            .and_then(StateValue::as_str)
            == Some("score")
        {
            let black = state_i64(state, RESULT_BLACK_SCORE_HALF);
            let white = state_i64(state, RESULT_WHITE_SCORE_HALF);
            let winner = state
                .ruleset_state
                .get(RESULT_WINNER)
                .and_then(StateValue::as_u64)
                .and_then(|value| u32::try_from(value).ok())
                .map(PlayerId::new);
            let margin = (black - white).unsigned_abs();
            return match winner {
                Some(player) => format!(
                    "{} wins by {}",
                    player_name(player),
                    format_half_points(i64::try_from(margin).unwrap_or(i64::MAX))
                ),
                None => "Jigo · tied score".to_owned(),
            };
        }
        if let Some(winner) = state
            .ruleset_state
            .get(RESULT_WINNER)
            .and_then(StateValue::as_u64)
            .and_then(|value| u32::try_from(value).ok())
            .map(PlayerId::new)
        {
            return format!("{} wins · resignation", player_name(winner));
        }
        return "Go finished".to_owned();
    }

    let active = state.turn.active_players.first().copied();
    match phase(state) {
        PHASE_PLAY => active
            .map(|player| format!("{} to play", player_name(player)))
            .unwrap_or_else(|| "Go".to_owned()),
        PHASE_REVIEW => {
            let prefix = if state_bool(state, REVIEW_DISAGREEMENT) {
                "Scoring dispute"
            } else {
                "Scoring review"
            };
            active
                .map(|player| format!("{prefix} · {} reviewing", player_name(player)))
                .unwrap_or_else(|| prefix.to_owned())
        }
        PHASE_FINAL_WHITE_PASS => "White must make the final AGA pass".to_owned(),
        _ => "Go".to_owned(),
    }
}

pub fn title(state: &GameState) -> String {
    format!("Go {}×{}", state.board.width(), state.board.height())
}

pub fn turn_notation(turn: &TurnRecord, board_size: u16) -> String {
    let Some(step) = turn.steps.last() else {
        return "turn".into();
    };
    match step.action.kind.as_str() {
        "go.place" => {
            let x = step.action.data.get("x").and_then(StateValue::as_u64);
            let y = step.action.data.get("y").and_then(StateValue::as_u64);
            match (x, y) {
                (Some(x), Some(y)) => format!(
                    "{}{}",
                    go_file(u16::try_from(x).unwrap_or(0)),
                    u16::try_from(y).unwrap_or(0).saturating_add(1).min(board_size)
                ),
                _ => "place".into(),
            }
        }
        "go.pass" => "pass".into(),
        "go.final_pass" => "final pass".into(),
        "go.mark_dead" => "mark dead group".into(),
        "go.review_done" => "done reviewing".into(),
        "go.resume" => "resume play".into(),
        "go.resign" => "resign".into(),
        other => other.strip_prefix("go.").unwrap_or(other).replace('_', " "),
    }
}

pub fn go_file(x: u16) -> String {
    let index = u32::from(x);
    let code = if index < 8 {
        u32::from(b'A') + index
    } else {
        u32::from(b'A') + index + 1
    };
    char::from_u32(code).unwrap_or('?').to_string()
}

pub fn format_half_points(value: i64) -> String {
    let sign = if value < 0 { "-" } else { "" };
    let absolute = value.unsigned_abs();
    if absolute % 2 == 0 {
        format!("{sign}{}", absolute / 2)
    } else {
        format!("{sign}{}.5", absolute / 2)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use monumla_core::{GameTimeline, InteractionDriver};

    fn add_stone(state: &mut GameState, id: u32, owner: PlayerId, x: u16, y: u16) {
        state
            .add_entity(EntityState::new(
                EntityId::new(id),
                STONE,
                owner,
                Position::new(x, y),
            ))
            .unwrap();
    }

    fn choice_at(driver: &InteractionDriver<GoInteractionRules>, position: Position) -> Choice {
        driver
            .interaction()
            .choices
            .iter()
            .find(|choice| {
                matches!(choice.kind, ChoiceKind::SelectPosition { position: found } if found == position)
            })
            .unwrap()
            .clone()
    }

    fn option(driver: &InteractionDriver<GoInteractionRules>, expected: &str) -> Choice {
        driver
            .interaction()
            .choices
            .iter()
            .find(|choice| matches!(&choice.kind, ChoiceKind::SelectOption { key } if key == expected))
            .unwrap()
            .clone()
    }

    fn commit_choice(
        timeline: &mut GameTimeline,
        choice_key: &str,
    ) -> Result<(), InteractionError> {
        let actor = *timeline.current().turn.active_players.first().unwrap();
        let turn = timeline.begin_turn(actor)?;
        let mut driver = InteractionDriver::new(
            GoInteractionRules::with_history(timeline.history())?,
            turn,
        )?;
        let choice = option(&driver, choice_key);
        driver.choose(choice.id)?;
        timeline.commit_turn(driver.into_turn()?)?;
        Ok(())
    }

    #[test]
    fn standard_config_is_current_aga_even_game() {
        let config = GoConfig::standard();
        assert_eq!(config.size, 19);
        assert_eq!(config.komi_half_points, 15);
        assert_eq!(config.scoring, GoScoring::Territory);
        assert_eq!(config.handicap, 0);
        let state = state_from_config(config).unwrap();
        assert_eq!(state.turn.active_players, vec![BLACK]);
        assert!(state.entities.is_empty());
    }

    #[test]
    fn fixed_aga_handicap_places_stones_and_white_moves_first() {
        let config = GoConfig::aga(19, GoScoring::Territory, 5).unwrap();
        let state = state_from_config(config).unwrap();
        assert_eq!(state.entities.len(), 5);
        assert_eq!(state.turn.active_players, vec![WHITE]);
        assert_eq!(state_i64(&state, CONFIG_KOMI_HALF), 1);
        assert!(state.entity_at(Position::new(9, 9)).unwrap().is_some());
    }

    #[test]
    fn one_stone_handicap_needs_no_setup_stone_on_smaller_boards() {
        let state = state_from_config(GoConfig::aga(9, GoScoring::Territory, 1).unwrap()).unwrap();
        assert!(state.entities.is_empty());
        assert_eq!(state.turn.active_players, vec![BLACK]);
        assert_eq!(state_i64(&state, CONFIG_KOMI_HALF), 1);
    }

    #[test]
    fn area_handicap_compensation_matches_aga_counting() {
        let mut state = state_from_config(GoConfig::aga(19, GoScoring::Area, 9).unwrap()).unwrap();
        let compensated = score_position(&state).unwrap();
        state.ruleset_state.insert(CONFIG_HANDICAP, 1_u64);
        let uncompensated = score_position(&state).unwrap();
        assert_eq!(
            compensated.white_half - uncompensated.white_half,
            16,
            "nine-stone area counting adds eight points to White beyond 0.5 komi"
        );
        assert_eq!(compensated.black_half, uncompensated.black_half);
    }

    #[test]
    fn placement_can_capture_a_multi_stone_group_without_move_semantics() {
        let mut state = empty_state(5);
        add_stone(&mut state, 1, WHITE, 2, 2);
        add_stone(&mut state, 2, WHITE, 2, 3);
        add_stone(&mut state, 3, BLACK, 1, 2);
        add_stone(&mut state, 4, BLACK, 3, 2);
        add_stone(&mut state, 5, BLACK, 2, 1);
        add_stone(&mut state, 6, BLACK, 1, 3);
        add_stone(&mut state, 7, BLACK, 3, 3);
        state.set_active_players(vec![BLACK]).unwrap();

        let turn = TurnSession::new(&state, BLACK).unwrap();
        let mut driver = InteractionDriver::new(GoInteractionRules::default(), turn).unwrap();
        let place = choice_at(&driver, Position::new(2, 4));
        driver.choose(place.id).unwrap();

        assert!(driver.turn().working.entity(EntityId::new(1)).is_err());
        assert!(driver.turn().working.entity(EntityId::new(2)).is_err());
        assert_eq!(state_u64(&driver.turn().working, PRISONERS_BLACK), 2);
        assert_eq!(driver.turn().working.turn.active_players, vec![WHITE]);
    }

    #[test]
    fn self_capture_is_rejected_after_opponent_capture_resolution() {
        let mut state = empty_state(3);
        add_stone(&mut state, 1, WHITE, 1, 0);
        add_stone(&mut state, 2, WHITE, 0, 1);
        add_stone(&mut state, 3, WHITE, 2, 1);
        add_stone(&mut state, 4, WHITE, 1, 2);
        state.set_active_players(vec![BLACK]).unwrap();
        assert!(!placement_is_legal(
            &state,
            &BTreeSet::new(),
            BLACK,
            Position::new(1, 1)
        )
        .unwrap());
    }

    #[test]
    fn superko_history_includes_play_situations_created_by_passes() {
        let mut timeline = GameTimeline::new(empty_state(3)).unwrap();
        let initial = board_key(timeline.current()).unwrap();
        commit_choice(&mut timeline, "pass").unwrap();
        let seen = historical_play_positions(Some(timeline.history())).unwrap();
        assert!(seen.contains(&(BLACK, initial)));
        assert!(seen.contains(&(WHITE, board_key(timeline.current()).unwrap())));
    }

    #[test]
    fn situational_superko_checks_all_prior_play_positions_not_only_previous_turn() {
        let mut state = empty_state(5);
        add_stone(&mut state, 1, BLACK, 1, 2);
        add_stone(&mut state, 2, BLACK, 3, 2);
        add_stone(&mut state, 3, BLACK, 2, 1);
        add_stone(&mut state, 4, WHITE, 2, 2);
        add_stone(&mut state, 5, WHITE, 1, 3);
        add_stone(&mut state, 6, WHITE, 3, 3);
        add_stone(&mut state, 7, WHITE, 2, 4);
        state.set_active_players(vec![BLACK]).unwrap();

        let mut timeline = GameTimeline::new(state).unwrap();
        let turn = timeline.begin_turn(BLACK).unwrap();
        let mut black = InteractionDriver::new(GoInteractionRules::default(), turn).unwrap();
        let capture = choice_at(&black, Position::new(2, 3));
        black.choose(capture.id).unwrap();
        timeline.commit_turn(black.into_turn().unwrap()).unwrap();

        // A pass is legal even though it repeats the board, but it must not erase
        // the earlier played position from the superko history.
        commit_choice(&mut timeline, "pass").unwrap();
        commit_choice(&mut timeline, "pass").unwrap();
        assert_eq!(phase(timeline.current()), PHASE_REVIEW);
        commit_choice(&mut timeline, "resume_play").unwrap();

        let actor = *timeline.current().turn.active_players.first().unwrap();
        let turn = timeline.begin_turn(actor).unwrap();
        let driver = InteractionDriver::new(
            GoInteractionRules::with_history(timeline.history()).unwrap(),
            turn,
        )
        .unwrap();
        assert!(!driver.interaction().choices.iter().any(|choice| {
            matches!(choice.kind, ChoiceKind::SelectPosition { position } if position == Position::new(2, 2))
        }));
    }

    #[test]
    fn pass_stones_are_prisoners_and_two_passes_enter_scoring_review() {
        let mut timeline = GameTimeline::new(empty_state(5)).unwrap();
        commit_choice(&mut timeline, "pass").unwrap();
        assert_eq!(state_u64(timeline.current(), PRISONERS_WHITE), 1);
        commit_choice(&mut timeline, "pass").unwrap();
        assert_eq!(state_u64(timeline.current(), PRISONERS_BLACK), 1);
        assert_eq!(phase(timeline.current()), PHASE_REVIEW);
        assert_eq!(timeline.current().turn.active_players, vec![BLACK]);
    }

    #[test]
    fn scoring_phase_and_pass_stones_roundtrip_through_undo_redo() {
        let mut timeline = GameTimeline::new(empty_state(5)).unwrap();
        commit_choice(&mut timeline, "pass").unwrap();
        commit_choice(&mut timeline, "pass").unwrap();
        assert_eq!(phase(timeline.current()), PHASE_REVIEW);
        assert_eq!(state_u64(timeline.current(), PRISONERS_BLACK), 1);
        assert_eq!(state_u64(timeline.current(), PRISONERS_WHITE), 1);

        timeline.undo().unwrap();
        assert_eq!(phase(timeline.current()), PHASE_PLAY);
        assert_eq!(timeline.current().turn.active_players, vec![WHITE]);
        assert_eq!(state_u64(timeline.current(), PRISONERS_BLACK), 0);
        assert_eq!(state_u64(timeline.current(), PRISONERS_WHITE), 1);

        timeline.redo().unwrap();
        assert_eq!(phase(timeline.current()), PHASE_REVIEW);
        assert_eq!(state_u64(timeline.current(), PRISONERS_BLACK), 1);
    }

    #[test]
    fn scoring_review_requires_matching_dead_group_assessments() {
        let mut state = empty_state(5);
        add_stone(&mut state, 1, WHITE, 2, 2);
        add_stone(&mut state, 2, BLACK, 1, 2);
        add_stone(&mut state, 3, BLACK, 3, 2);
        add_stone(&mut state, 4, BLACK, 2, 1);
        add_stone(&mut state, 5, BLACK, 2, 3);
        state.ruleset_state.insert(PHASE, PHASE_REVIEW);
        state.ruleset_state.insert(LAST_PASSER, u64::from(WHITE.get()));
        state.set_active_players(vec![BLACK]).unwrap();
        let mut timeline = GameTimeline::new(state).unwrap();

        let turn = timeline.begin_turn(BLACK).unwrap();
        let mut black = InteractionDriver::new(GoInteractionRules::default(), turn).unwrap();
        let mark = black
            .interaction()
            .choices
            .iter()
            .find(|choice| matches!(choice.kind, ChoiceKind::SelectEntity { entity } if entity == EntityId::new(1)))
            .unwrap()
            .clone();
        black.choose(mark.id).unwrap();
        timeline.commit_turn(black.into_turn().unwrap()).unwrap();
        commit_choice(&mut timeline, "review_done").unwrap();
        commit_choice(&mut timeline, "review_done").unwrap();
        assert_eq!(phase(timeline.current()), PHASE_REVIEW);
        assert!(state_bool(timeline.current(), REVIEW_DISAGREEMENT));
    }

    #[test]
    fn agreed_dead_stones_are_removed_and_count_as_prisoners() {
        let mut state = empty_state(3);
        add_stone(&mut state, 1, WHITE, 1, 1);
        add_stone(&mut state, 2, BLACK, 0, 1);
        add_stone(&mut state, 3, BLACK, 2, 1);
        add_stone(&mut state, 4, BLACK, 1, 0);
        add_stone(&mut state, 5, BLACK, 1, 2);
        for entity in group_rule(&state, EntityId::new(1)).unwrap() {
            state.entity_mut(entity).unwrap().state.insert(DEAD_MARK_BLACK, true);
            state.entity_mut(entity).unwrap().state.insert(DEAD_MARK_WHITE, true);
        }
        state.ruleset_state.insert(PHASE, PHASE_REVIEW);
        state.ruleset_state.insert(LAST_PASSER, u64::from(WHITE.get()));
        state.set_active_players(vec![BLACK]).unwrap();
        finish_scoring(&mut state, FINALIZE_AGREED).unwrap();
        assert!(state.entity(EntityId::new(1)).is_err());
        assert_eq!(state_u64(&state, PRISONERS_BLACK), 1);
        assert_eq!(phase(&state), PHASE_FINISHED);
    }

    #[test]
    fn territory_scoring_uses_aga_prisoner_subtraction_for_absolute_scores() {
        let mut state = empty_state(3);
        state.ruleset_state.insert(PRISONERS_BLACK, 2_u64);
        state.ruleset_state.insert(PRISONERS_WHITE, 3_u64);
        let score = score_position(&state).unwrap();
        assert_eq!(score.black_half, -6);
        assert_eq!(score.white_half, 11);
    }

    #[test]
    fn changing_one_review_assessment_does_not_invalidate_the_other_players_submission() {
        let mut state = empty_state(3);
        add_stone(&mut state, 1, WHITE, 1, 1);
        state.ruleset_state.insert(PHASE, PHASE_REVIEW);
        state.ruleset_state.insert(REVIEW_DONE_BLACK, true);
        state.ruleset_state.insert(REVIEW_DONE_WHITE, true);
        toggle_group_mark(&mut state, WHITE, EntityId::new(1)).unwrap();
        assert!(state_bool(&state, REVIEW_DONE_BLACK));
        assert!(!state_bool(&state, REVIEW_DONE_WHITE));
    }

    #[test]
    fn area_and_territory_scoring_are_both_supported_with_exact_half_point_komi() {
        let mut territory_state = state_from_config(
            GoConfig::aga(3, GoScoring::Territory, 0).unwrap(),
        )
        .unwrap();
        add_stone(&mut territory_state, 1, BLACK, 0, 0);
        add_stone(&mut territory_state, 2, BLACK, 0, 1);
        add_stone(&mut territory_state, 3, BLACK, 1, 0);
        add_stone(&mut territory_state, 4, WHITE, 2, 2);
        let territory_score = score_position(&territory_state).unwrap();
        assert_eq!(territory_score.white_half % 2, 1);

        let mut area_state = territory_state.clone();
        area_state.ruleset_state.insert(CONFIG_SCORING, "area");
        let area_score = score_position(&area_state).unwrap();
        assert_ne!(area_score.black_half, territory_score.black_half);
    }

    #[test]
    fn black_last_requires_an_explicit_white_final_pass() {
        let mut state = empty_state(5);
        state.ruleset_state.insert(PHASE, PHASE_REVIEW);
        state.ruleset_state.insert(LAST_PASSER, u64::from(BLACK.get()));
        state.ruleset_state.insert(REVIEW_DONE_BLACK, true);
        state.set_active_players(vec![WHITE]).unwrap();
        let mut timeline = GameTimeline::new(state).unwrap();
        commit_choice(&mut timeline, "review_done").unwrap();
        assert_eq!(phase(timeline.current()), PHASE_FINAL_WHITE_PASS);
        assert_eq!(timeline.current().turn.active_players, vec![WHITE]);
        commit_choice(&mut timeline, "final_pass").unwrap();
        assert_eq!(phase(timeline.current()), PHASE_FINISHED);
        assert_eq!(state_u64(timeline.current(), PRISONERS_BLACK), 1);
    }

    #[test]
    fn dispute_can_resume_and_immediate_two_passes_finish_with_all_stones_alive() {
        let mut state = empty_state(5);
        state.ruleset_state.insert(PHASE, PHASE_REVIEW);
        state.ruleset_state.insert(LAST_PASSER, u64::from(WHITE.get()));
        state.ruleset_state.insert(REVIEW_DISAGREEMENT, true);
        state.set_active_players(vec![BLACK]).unwrap();
        let mut timeline = GameTimeline::new(state).unwrap();
        commit_choice(&mut timeline, "resume_play").unwrap();
        assert!(state_bool(timeline.current(), RESUMED_AFTER_DISPUTE));
        commit_choice(&mut timeline, "pass").unwrap();
        commit_choice(&mut timeline, "pass").unwrap();
        assert_eq!(phase(timeline.current()), PHASE_FINISHED);
    }

    #[test]
    fn resignation_is_a_terminal_undoable_ruleset_outcome() {
        let mut timeline = GameTimeline::new(empty_state(9)).unwrap();
        commit_choice(&mut timeline, "resign").unwrap();
        let outcome = registry()
            .outcome(RuleContext::from_state(
                timeline.current(),
                Some(timeline.history()),
            ))
            .unwrap()
            .unwrap();
        assert_eq!(outcome.key, "go.resignation");
        assert_eq!(outcome.winners, vec![WHITE]);
        timeline.undo().unwrap();
        assert!(registry()
            .outcome(RuleContext::from_state(
                timeline.current(),
                Some(timeline.history())
            ))
            .unwrap()
            .is_none());
    }

    #[test]
    fn go_coordinates_skip_i() {
        assert_eq!(go_file(0), "A");
        assert_eq!(go_file(7), "H");
        assert_eq!(go_file(8), "J");
        assert_eq!(go_file(18), "T");
    }

    #[test]
    fn stone_presentation_exposes_scoring_marks_without_core_special_cases() {
        let mut state = empty_state(5);
        add_stone(&mut state, 1, BLACK, 0, 0);
        state
            .entity_mut(EntityId::new(1))
            .unwrap()
            .state
            .insert(DEAD_MARK_WHITE, true);
        let presentation = registry()
            .presentation(
                RuleContext::from_state(&state, None),
                EntityId::new(1),
            )
            .unwrap();
        assert_eq!(presentation.asset_key, "go/black/stone");
        assert_eq!(
            presentation
                .data
                .get("disputed_dead")
                .and_then(StateValue::as_bool),
            Some(true)
        );
    }

    #[test]
    fn board_keys_ignore_ruleset_metadata_but_preserve_stone_ownership() {
        let mut a = empty_state(3);
        let mut b = a.clone();
        a.ruleset_state.insert("irrelevant", 1_u64);
        b.ruleset_state.insert("irrelevant", 2_u64);
        assert_eq!(board_key_rule(&a).unwrap(), board_key_rule(&b).unwrap());
        add_stone(&mut b, 1, BLACK, 1, 1);
        assert_ne!(board_key_rule(&a).unwrap(), board_key_rule(&b).unwrap());
    }
}

#[cfg(test)]
mod legality_tests;
