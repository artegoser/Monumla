use crate::{
    pieces::pawn::Pawn, ChessError, ChessMoveKind, ChessOutcome, ChessRules, ChessSide, PseudoMove,
    BISHOP, KING, KNIGHT, PAWN, QUEEN, ROOK, STANDARD_FEN, WHITE_PLAYER,
};
use monumla_core::{
    ChoiceInput, EntityId, EntityTypeId, GameState, GameTimeline, History, Position, RecordedAction,
    StateMap, StateValue, TurnRecord, TurnSession,
};
use std::collections::BTreeMap;

#[derive(Clone, Debug)]
pub struct PgnGame {
    pub timeline: GameTimeline,
    pub tags: BTreeMap<String, String>,
    pub initial_fen: String,
}

impl ChessRules {
    pub fn san_for_move(
        &self,
        state: &GameState,
        history: &History,
        movement: PseudoMove,
        promotion: Option<EntityTypeId>,
    ) -> Result<String, ChessError> {
        let actor = state.entity(movement.actor)?;
        let side = ChessSide::from_player(actor.owner).ok_or(ChessError::UnknownSide(actor.owner))?;
        let mut san = match movement.kind {
            ChessMoveKind::Castle { .. } => {
                if movement.to.x > movement.from.x {
                    "O-O".to_owned()
                } else {
                    "O-O-O".to_owned()
                }
            }
            _ => {
                let mut value = String::new();
                if actor.entity_type == PAWN {
                    if movement.capture.is_some() {
                        value.push(file_char(movement.from.x)?);
                    }
                } else {
                    value.push(piece_letter(actor.entity_type).ok_or_else(|| {
                        ChessError::InvalidSan("unknown chess piece type".into())
                    })?);
                    value.push_str(&self.san_disambiguation(state, history, movement)?);
                }
                if movement.capture.is_some() {
                    value.push('x');
                }
                value.push_str(&square_name(movement.to)?);
                let local_choices = self.piece_move_choices(state, Some(history), movement)?;
                let move_choices =
                    self.move_choices(state, Some(history), movement, &StateMap::new())?;
                if !local_choices.is_empty() && move_choices.is_empty() {
                    return Err(ChessError::MoveInputRejected(movement.actor));
                }
                if !move_choices.is_empty() {
                    let promotion = promotion.ok_or(ChessError::PromotionRequired(movement.actor))?;
                    let letter = piece_letter(promotion)
                        .filter(|_| Pawn::is_promotion_type(promotion))
                        .ok_or(ChessError::InvalidPromotion(promotion))?;
                    value.push('=');
                    value.push(letter);
                } else if promotion.is_some() {
                    return Err(ChessError::UnexpectedPromotion(movement.actor));
                }
                value
            }
        };

        let (after, next_history) = self.simulate_notated_turn(state, history, movement, promotion)?;
        let next_side = side.opponent();
        if self.in_check(&after, next_side)? {
            let replies = self.legal_moves_for_side_with_history(&after, Some(&next_history), next_side)?;
            san.push(if replies.is_empty() { '#' } else { '+' });
        }
        Ok(san)
    }

    pub fn san_for_turn(&self, turn: &TurnRecord, prefix: &History) -> Result<String, ChessError> {
        if turn.synthetic {
            return Err(ChessError::InvalidSan(
                "synthetic compatibility turns do not have SAN".into(),
            ));
        }
        let step = turn
            .steps
            .last()
            .ok_or_else(|| ChessError::InvalidSan("chess turn has no recorded step".into()))?;
        if step.action.kind != "chess_move" {
            return Err(ChessError::InvalidSan(format!(
                "unsupported recorded action {}",
                step.action.kind
            )));
        }
        let actor = state_entity_id(&step.action.data, "actor")?;
        let from = action_position(&step.action.data, "from_x", "from_y")?;
        let to = action_position(&step.action.data, "to_x", "to_y")?;
        let before_actor = turn.before.entity(actor)?;
        let after_actor = turn.after.entity(actor)?;
        let promotion = (before_actor.entity_type == PAWN && after_actor.entity_type != PAWN)
            .then_some(after_actor.entity_type);
        let movement = self
            .legal_moves_with_history(&turn.before, Some(prefix), actor)?
            .into_iter()
            .find(|movement| movement.from == from && movement.to == to)
            .ok_or(ChessError::IllegalMove(actor, to))?;
        self.san_for_move(&turn.before, prefix, movement, promotion)
    }

    pub fn resolve_san(
        &self,
        state: &GameState,
        history: &History,
        san: &str,
    ) -> Result<(PseudoMove, Option<EntityTypeId>), ChessError> {
        let token = normalize_san(san);
        let side = self.side_to_move(state)?;
        let moves = self.legal_moves_for_side_with_history(state, Some(history), side)?;
        let mut matches = Vec::new();
        for movement in moves {
            let local_choices = self.piece_move_choices(state, Some(history), movement)?;
            let move_choices = self.move_choices(state, Some(history), movement, &StateMap::new())?;
            if !local_choices.is_empty() && move_choices.is_empty() {
                continue;
            }
            if move_choices.is_empty() {
                let candidate = normalize_san(&self.san_for_move(state, history, movement, None)?);
                if candidate == token {
                    matches.push((movement, None));
                }
                continue;
            }

            for choice in move_choices {
                let Some(promotion) = choice
                    .data
                    .get("entity_type")
                    .and_then(StateValue::as_u64)
                    .and_then(|value| u32::try_from(value).ok())
                    .map(EntityTypeId::new)
                else {
                    continue;
                };
                let candidate = normalize_san(&self.san_for_move(
                    state,
                    history,
                    movement,
                    Some(promotion),
                )?);
                if candidate == token {
                    matches.push((movement, Some(promotion)));
                }
            }
        }
        match matches.len() {
            0 => Err(ChessError::InvalidSan(san.trim().to_owned())),
            1 => Ok(matches[0]),
            _ => Err(ChessError::AmbiguousSan(san.trim().to_owned())),
        }
    }

    pub fn from_pgn(&self, pgn: &str) -> Result<PgnGame, ChessError> {
        let (tags, movetext) = parse_pgn_document(pgn)?;
        let initial_fen = tags
            .get("FEN")
            .cloned()
            .unwrap_or_else(|| STANDARD_FEN.to_owned());
        let imported = self.from_fen(&initial_fen)?;
        let mut timeline = imported.timeline;

        let (tokens, movetext_result) = pgn_san_tokens(&movetext)?;
        for token in tokens {
            let history = timeline.history().clone();
            let state = timeline.current().clone();
            let (movement, promotion) = self.resolve_san(&state, &history, &token)?;
            let side = self.side_to_move(&state)?;
            let mut turn = timeline.begin_turn(side.player())?;
            let input = self.notation_move_input(&state, &history, movement, promotion)?;
            self.execute_move(&mut turn, Some(&history), movement, input.as_ref())?;
            timeline.commit_turn(turn)?;
        }

        let tag_result = tags.get("Result").map(String::as_str);
        if tag_result.is_some_and(|token| !is_result_token(token)) {
            return Err(ChessError::InvalidPgn("invalid Result tag".into()));
        }
        if let (Some(tag), Some(movetext)) = (tag_result, movetext_result.as_deref()) {
            if tag != movetext {
                return Err(ChessError::InvalidPgn(format!(
                    "Result tag {tag:?} conflicts with movetext result {movetext:?}"
                )));
            }
        }
        let declared_result = movetext_result
            .as_deref()
            .or(tag_result)
            .unwrap_or("*");
        let result_was_explicit = movetext_result.is_some() || tag_result.is_some();
        self.apply_declared_pgn_result(&mut timeline, declared_result, result_was_explicit)?;

        Ok(PgnGame {
            timeline,
            tags,
            initial_fen,
        })
    }

    pub fn to_pgn(&self, initial_fen: &str, history: &History) -> Result<String, ChessError> {
        self.to_pgn_with_tags(initial_fen, history, &BTreeMap::new())
    }

    pub fn to_pgn_with_tags(
        &self,
        initial_fen: &str,
        history: &History,
        source_tags: &BTreeMap<String, String>,
    ) -> Result<String, ChessError> {
        let initial = self.from_fen(initial_fen)?;
        let mut prefix = History::default();
        let mut moves = Vec::new();
        let mut final_state = initial.timeline.current().clone();

        for turn in history.turns() {
            if turn.synthetic {
                prefix = prefix.with_appended(turn.clone())?;
                final_state = turn.after.clone();
                continue;
            }
            if turn.steps.last().is_some_and(|step| step.action.kind == "chess_move") {
                let san = self.san_for_turn(turn, &prefix)?;
                let side = ChessSide::from_player(turn.actor).ok_or(ChessError::UnknownSide(turn.actor))?;
                moves.push((self.fullmove_number(&turn.before), side, san));
            }
            prefix = prefix.with_appended(turn.clone())?;
            final_state = turn.after.clone();
        }

        let result = result_token(self.status(&final_state, history)?.outcome.as_ref());
        let mut tags = vec![
            ("Event".to_owned(), source_tags.get("Event").cloned().unwrap_or_else(|| "Monumla local chess game".to_owned())),
            ("Site".to_owned(), source_tags.get("Site").cloned().unwrap_or_else(|| "?".to_owned())),
            ("Date".to_owned(), source_tags.get("Date").cloned().unwrap_or_else(|| "????.??.??".to_owned())),
            ("Round".to_owned(), source_tags.get("Round").cloned().unwrap_or_else(|| "-".to_owned())),
            ("White".to_owned(), source_tags.get("White").cloned().unwrap_or_else(|| "White".to_owned())),
            ("Black".to_owned(), source_tags.get("Black").cloned().unwrap_or_else(|| "Black".to_owned())),
            ("Result".to_owned(), result.to_owned()),
        ];
        if initial_fen != STANDARD_FEN {
            tags.push(("SetUp".to_owned(), "1".to_owned()));
            tags.push(("FEN".to_owned(), initial_fen.to_owned()));
        }
        for (name, value) in source_tags {
            if matches!(name.as_str(), "Event" | "Site" | "Date" | "Round" | "White" | "Black" | "Result" | "SetUp" | "FEN") {
                continue;
            }
            tags.push((name.clone(), value.clone()));
        }

        let mut output = String::new();
        for (name, value) in tags {
            output.push('[');
            output.push_str(&name);
            output.push_str(" \"");
            output.push_str(&escape_pgn_tag(&value));
            output.push_str("\"]\n");
        }
        output.push('\n');

        let mut previous: Option<(u32, ChessSide)> = None;
        for (number, side, san) in moves {
            match side {
                ChessSide::White => {
                    if !output.ends_with('\n') && !output.ends_with(' ') {
                        output.push(' ');
                    }
                    output.push_str(&format!("{number}. {san}"));
                }
                ChessSide::Black => {
                    if previous == Some((number, ChessSide::White)) {
                        output.push(' ');
                        output.push_str(&san);
                    } else {
                        if !output.ends_with('\n') && !output.ends_with(' ') {
                            output.push(' ');
                        }
                        output.push_str(&format!("{number}... {san}"));
                    }
                }
            }
            previous = Some((number, side));
        }
        if !output.ends_with('\n') && !output.ends_with(' ') {
            output.push(' ');
        }
        output.push_str(result);
        output.push('\n');
        Ok(output)
    }

    fn apply_declared_pgn_result(
        &self,
        timeline: &mut GameTimeline,
        declared: &str,
        declared_was_explicit: bool,
    ) -> Result<(), ChessError> {
        let status = self.status(timeline.current(), timeline.history())?;
        let natural = result_token(status.outcome.as_ref());
        if natural != "*" {
            if declared_was_explicit && declared != natural {
                return Err(ChessError::InvalidPgn(format!(
                    "declared result {declared:?} conflicts with board outcome {natural:?}"
                )));
            }
            return Ok(());
        }
        if declared == "*" {
            return Ok(());
        }

        let winner = match declared {
            "1-0" => Some(WHITE_PLAYER),
            "0-1" => Some(crate::BLACK_PLAYER),
            "1/2-1/2" => None,
            _ => return Err(ChessError::InvalidPgn("invalid game result".into())),
        };
        let actor = self.side_to_move(timeline.current())?.player();
        let mut turn = timeline.begin_turn(actor)?;
        turn.apply_transaction(
            RecordedAction::new("chess_recorded_result"),
            |transaction| -> Result<(), ChessError> {
                self.record_result(transaction.raw_state_mut(), winner)?;
                Ok(())
            },
        )?;
        timeline.commit_turn(turn)?;
        Ok(())
    }

    fn san_disambiguation(
        &self,
        state: &GameState,
        history: &History,
        movement: PseudoMove,
    ) -> Result<String, ChessError> {
        let actor = state.entity(movement.actor)?;
        let mut competitors = Vec::new();
        for entity in state.entities.values().filter(|entity| {
            entity.id != actor.id
                && entity.owner == actor.owner
                && entity.entity_type == actor.entity_type
        }) {
            if self
                .legal_moves_with_history(state, Some(history), entity.id)?
                .iter()
                .any(|candidate| candidate.to == movement.to)
            {
                competitors.push(entity);
            }
        }
        if competitors.is_empty() {
            return Ok(String::new());
        }
        let same_file = competitors
            .iter()
            .any(|entity| entity.position.x == movement.from.x);
        let same_rank = competitors
            .iter()
            .any(|entity| entity.position.y == movement.from.y);
        let mut value = String::new();
        if !same_file {
            value.push(file_char(movement.from.x)?);
        } else if !same_rank {
            value.push(rank_char(movement.from.y)?);
        } else {
            value.push(file_char(movement.from.x)?);
            value.push(rank_char(movement.from.y)?);
        }
        Ok(value)
    }

    fn notation_move_input(
        &self,
        state: &GameState,
        history: &History,
        movement: PseudoMove,
        promotion: Option<EntityTypeId>,
    ) -> Result<Option<ChoiceInput>, ChessError> {
        let local_choices = self.piece_move_choices(state, Some(history), movement)?;
        let choices = self.move_choices(state, Some(history), movement, &StateMap::new())?;
        if !local_choices.is_empty() && choices.is_empty() {
            return Err(ChessError::MoveInputRejected(movement.actor));
        }
        match promotion {
            None if choices.is_empty() => Ok(None),
            None => Err(ChessError::PromotionRequired(movement.actor)),
            Some(entity_type) => {
                let choice = choices
                    .into_iter()
                    .find(|choice| {
                        choice
                            .data
                            .get("entity_type")
                            .and_then(StateValue::as_u64)
                            == Some(u64::from(entity_type.get()))
                    })
                    .ok_or(ChessError::InvalidPromotion(entity_type))?;
                Ok(Some(ChoiceInput::from(&choice)))
            }
        }
    }

    pub(crate) fn simulate_notated_turn(
        &self,
        state: &GameState,
        history: &History,
        movement: PseudoMove,
        promotion: Option<EntityTypeId>,
    ) -> Result<(GameState, History), ChessError> {
        let actor = state.entity(movement.actor)?;
        let side = ChessSide::from_player(actor.owner).ok_or(ChessError::UnknownSide(actor.owner))?;
        let mut turn = TurnSession::new(state, side.player())?;
        let input = self.notation_move_input(state, history, movement, promotion)?;
        self.execute_move(&mut turn, Some(history), movement, input.as_ref())?;
        let record = TurnRecord {
            actor: turn.actor,
            before: turn.before.clone(),
            steps: turn.steps.clone(),
            after: turn.working.clone(),
            synthetic: false,
        };
        let next_history = history.with_appended(record)?;
        Ok((turn.working, next_history))
    }
}

fn state_entity_id(data: &monumla_core::StateMap, key: &str) -> Result<EntityId, ChessError> {
    data.get(key)
        .and_then(StateValue::as_u64)
        .and_then(|value| u32::try_from(value).ok())
        .map(EntityId::new)
        .ok_or_else(|| ChessError::InvalidSan(format!("recorded move has no {key}")))
}

fn action_position(
    data: &monumla_core::StateMap,
    x_key: &str,
    y_key: &str,
) -> Result<Position, ChessError> {
    let x = data
        .get(x_key)
        .and_then(StateValue::as_u64)
        .and_then(|value| u16::try_from(value).ok())
        .ok_or_else(|| ChessError::InvalidSan(format!("recorded move has no {x_key}")))?;
    let y = data
        .get(y_key)
        .and_then(StateValue::as_u64)
        .and_then(|value| u16::try_from(value).ok())
        .ok_or_else(|| ChessError::InvalidSan(format!("recorded move has no {y_key}")))?;
    Ok(Position::new(x, y))
}

fn piece_letter(entity_type: EntityTypeId) -> Option<char> {
    if entity_type == KNIGHT {
        Some('N')
    } else if entity_type == BISHOP {
        Some('B')
    } else if entity_type == ROOK {
        Some('R')
    } else if entity_type == QUEEN {
        Some('Q')
    } else if entity_type == KING {
        Some('K')
    } else {
        None
    }
}

fn file_char(file: u16) -> Result<char, ChessError> {
    let file = u8::try_from(file).map_err(|_| ChessError::InvalidSan("file is outside board".into()))?;
    if file >= 8 {
        return Err(ChessError::InvalidSan("file is outside chess board".into()));
    }
    Ok(char::from(b'a' + file))
}

fn rank_char(rank: u16) -> Result<char, ChessError> {
    let rank = u8::try_from(rank).map_err(|_| ChessError::InvalidSan("rank is outside board".into()))?;
    if rank >= 8 {
        return Err(ChessError::InvalidSan("rank is outside chess board".into()));
    }
    Ok(char::from(b'1' + rank))
}

fn square_name(position: Position) -> Result<String, ChessError> {
    Ok(format!("{}{}", file_char(position.x)?, rank_char(position.y)?))
}

fn normalize_san(value: &str) -> String {
    let mut value = value.trim().replace('0', "O");
    if let Some(index) = value.find('$') {
        value.truncate(index);
    }
    while matches!(value.chars().last(), Some('!' | '?' | '+' | '#')) {
        value.pop();
    }
    if value.ends_with("e.p.") {
        value.truncate(value.len().saturating_sub(4));
        value = value.trim().to_owned();
    }
    value
}

fn result_token(outcome: Option<&ChessOutcome>) -> &'static str {
    match outcome {
        Some(ChessOutcome::Checkmate { winner, .. })
        | Some(ChessOutcome::Resignation { winner, .. })
        | Some(ChessOutcome::RecordedWin { winner, .. }) => {
            if *winner == WHITE_PLAYER {
                "1-0"
            } else {
                "0-1"
            }
        }
        Some(
            ChessOutcome::Stalemate
            | ChessOutcome::DrawAgreement
            | ChessOutcome::ThreefoldRepetition
            | ChessOutcome::FivefoldRepetition
            | ChessOutcome::FiftyMoveRule
            | ChessOutcome::SeventyFiveMoveRule
            | ChessOutcome::DeadPosition
            | ChessOutcome::RecordedDraw,
        ) => "1/2-1/2",
        None => "*",
    }
}

fn escape_pgn_tag(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn parse_pgn_document(pgn: &str) -> Result<(BTreeMap<String, String>, String), ChessError> {
    let mut tags = BTreeMap::new();
    let mut movetext = String::new();
    let mut in_tags = true;
    for line in pgn.lines() {
        let trimmed = line.trim();
        if in_tags && trimmed.starts_with('[') {
            let (name, value) = parse_tag_line(trimmed)?;
            tags.insert(name, value);
        } else {
            if !trimmed.is_empty() {
                in_tags = false;
            }
            movetext.push_str(line);
            movetext.push('\n');
        }
    }
    Ok((tags, movetext))
}

fn parse_tag_line(line: &str) -> Result<(String, String), ChessError> {
    if !line.ends_with(']') {
        return Err(ChessError::InvalidPgn(format!("invalid tag line {line}")));
    }
    let inner = line[1..line.len() - 1].trim();
    let Some(space) = inner.find(char::is_whitespace) else {
        return Err(ChessError::InvalidPgn(format!("invalid tag line {line}")));
    };
    let name = inner[..space].trim();
    let raw = inner[space..].trim();
    if !raw.starts_with('"') || !raw.ends_with('"') {
        return Err(ChessError::InvalidPgn(format!("invalid tag value {line}")));
    }
    let mut value = String::new();
    let mut escaped = false;
    for ch in raw[1..raw.len() - 1].chars() {
        if escaped {
            value.push(ch);
            escaped = false;
        } else if ch == '\\' {
            escaped = true;
        } else {
            value.push(ch);
        }
    }
    if escaped {
        return Err(ChessError::InvalidPgn("unterminated tag escape".into()));
    }
    Ok((name.to_owned(), value))
}

fn pgn_san_tokens(movetext: &str) -> Result<(Vec<String>, Option<String>), ChessError> {
    let stripped = strip_pgn_comments_and_variations(movetext)?;
    let mut moves = Vec::new();
    let mut result = None;
    for raw in stripped.split_whitespace() {
        if raw.starts_with('$') || raw.eq_ignore_ascii_case("e.p.") {
            continue;
        }
        let token = strip_move_number_prefix(raw);
        if token.is_empty() || token.starts_with('$') {
            continue;
        }
        if is_result_token(token) {
            if result.as_deref().is_some_and(|previous| previous != token) {
                return Err(ChessError::InvalidPgn("conflicting movetext results".into()));
            }
            result = Some(token.to_owned());
            continue;
        }
        if result.is_some() {
            return Err(ChessError::InvalidPgn(
                "SAN move appears after the game result".into(),
            ));
        }
        moves.push(token.to_owned());
    }
    Ok((moves, result))
}

fn strip_move_number_prefix(mut token: &str) -> &str {
    loop {
        let digit_count = token.chars().take_while(|ch| ch.is_ascii_digit()).count();
        if digit_count == 0 {
            return token;
        }
        let rest = &token[digit_count..];
        let dot_count = rest.chars().take_while(|ch| *ch == '.').count();
        if dot_count == 0 {
            return token;
        }
        token = &rest[dot_count..];
        if token.is_empty() {
            return token;
        }
    }
}

fn is_result_token(token: &str) -> bool {
    matches!(token, "1-0" | "0-1" | "1/2-1/2" | "*")
}

fn strip_pgn_comments_and_variations(input: &str) -> Result<String, ChessError> {
    let mut output = String::with_capacity(input.len());
    let mut brace_depth = 0_u32;
    let mut variation_depth = 0_u32;
    let mut semicolon_comment = false;

    for ch in input.chars() {
        if semicolon_comment {
            if ch == '\n' {
                semicolon_comment = false;
                if variation_depth == 0 {
                    output.push(' ');
                }
            }
            continue;
        }

        if brace_depth > 0 {
            if ch == '{' {
                brace_depth += 1;
            } else if ch == '}' {
                brace_depth -= 1;
            }
            continue;
        }

        match ch {
            '{' => {
                if variation_depth == 0
                    && !output.chars().last().is_some_and(char::is_whitespace)
                {
                    output.push(' ');
                }
                brace_depth = 1;
            }
            ';' => {
                if variation_depth == 0
                    && !output.chars().last().is_some_and(char::is_whitespace)
                {
                    output.push(' ');
                }
                semicolon_comment = true;
            }
            '(' => {
                if variation_depth == 0
                    && !output.chars().last().is_some_and(char::is_whitespace)
                {
                    output.push(' ');
                }
                variation_depth += 1;
            }
            ')' => {
                if variation_depth == 0 {
                    return Err(ChessError::InvalidPgn("unmatched variation close".into()));
                }
                variation_depth -= 1;
            }
            _ if variation_depth > 0 => {}
            _ => output.push(ch),
        }
    }

    if brace_depth != 0 || variation_depth != 0 {
        return Err(ChessError::InvalidPgn(
            "unterminated PGN comment or variation".into(),
        ));
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn move_to(
        rules: &ChessRules,
        state: &GameState,
        history: &History,
        from: Position,
        to: Position,
    ) -> PseudoMove {
        let actor = state.entity_at(from).unwrap().unwrap().id;
        rules
            .legal_moves_with_history(state, Some(history), actor)
            .unwrap()
            .into_iter()
            .find(|movement| movement.to == to)
            .unwrap()
    }

    #[test]
    fn san_covers_quiet_capture_castling_promotion_and_checkmate() {
        let rules = ChessRules::standard();

        let imported = rules.from_fen(STANDARD_FEN).unwrap();
        let movement = move_to(
            &rules,
            imported.timeline.current(),
            imported.timeline.history(),
            Position::new(4, 1),
            Position::new(4, 3),
        );
        assert_eq!(
            rules
                .san_for_move(imported.timeline.current(), imported.timeline.history(), movement, None)
                .unwrap(),
            "e4"
        );

        let imported = rules
            .from_fen("4k3/8/8/3p4/4P3/8/8/4K3 w - - 0 1")
            .unwrap();
        let movement = move_to(
            &rules,
            imported.timeline.current(),
            imported.timeline.history(),
            Position::new(4, 3),
            Position::new(3, 4),
        );
        assert_eq!(
            rules
                .san_for_move(imported.timeline.current(), imported.timeline.history(), movement, None)
                .unwrap(),
            "exd5"
        );

        let imported = rules
            .from_fen("4k3/8/8/8/8/8/8/4K2R w K - 0 1")
            .unwrap();
        let movement = move_to(
            &rules,
            imported.timeline.current(),
            imported.timeline.history(),
            Position::new(4, 0),
            Position::new(6, 0),
        );
        assert_eq!(
            rules
                .san_for_move(imported.timeline.current(), imported.timeline.history(), movement, None)
                .unwrap(),
            "O-O"
        );

        let imported = rules
            .from_fen("4k3/P7/8/8/8/8/8/4K3 w - - 0 1")
            .unwrap();
        let movement = move_to(
            &rules,
            imported.timeline.current(),
            imported.timeline.history(),
            Position::new(0, 6),
            Position::new(0, 7),
        );
        assert_eq!(
            rules
                .san_for_move(
                    imported.timeline.current(),
                    imported.timeline.history(),
                    movement,
                    Some(QUEEN),
                )
                .unwrap(),
            "a8=Q+"
        );

        let game = rules.from_pgn("1. f3 e5 2. g4 Qh4#").unwrap();
        let last = game
            .timeline
            .history()
            .turns()
            .iter()
            .rev()
            .find(|turn| !turn.synthetic)
            .unwrap();
        let mut prefix = History::default();
        for turn in game.timeline.history().turns() {
            if std::ptr::eq(turn, last) {
                break;
            }
            prefix = prefix.with_appended(turn.clone()).unwrap();
        }
        assert_eq!(rules.san_for_turn(last, &prefix).unwrap(), "Qh4#");
    }

    #[test]
    fn san_disambiguates_by_file_and_rank() {
        let rules = ChessRules::standard();
        let imported = rules
            .from_fen("4k3/8/8/8/8/8/8/1N2KN2 w - - 0 1")
            .unwrap();
        let left = move_to(
            &rules,
            imported.timeline.current(),
            imported.timeline.history(),
            Position::new(1, 0),
            Position::new(3, 1),
        );
        let right = move_to(
            &rules,
            imported.timeline.current(),
            imported.timeline.history(),
            Position::new(5, 0),
            Position::new(3, 1),
        );
        assert_eq!(
            rules
                .san_for_move(imported.timeline.current(), imported.timeline.history(), left, None)
                .unwrap(),
            "Nbd2"
        );
        assert_eq!(
            rules
                .san_for_move(imported.timeline.current(), imported.timeline.history(), right, None)
                .unwrap(),
            "Nfd2"
        );

        let imported = rules
            .from_fen("4k3/8/8/8/8/R7/8/R3K3 w - - 0 1")
            .unwrap();
        let lower = move_to(
            &rules,
            imported.timeline.current(),
            imported.timeline.history(),
            Position::new(0, 0),
            Position::new(0, 1),
        );
        let upper = move_to(
            &rules,
            imported.timeline.current(),
            imported.timeline.history(),
            Position::new(0, 2),
            Position::new(0, 1),
        );
        assert_eq!(
            rules
                .san_for_move(imported.timeline.current(), imported.timeline.history(), lower, None)
                .unwrap(),
            "R1a2"
        );
        assert_eq!(
            rules
                .san_for_move(imported.timeline.current(), imported.timeline.history(), upper, None)
                .unwrap(),
            "R3a2"
        );

        let imported = rules
            .from_fen("4k3/8/8/8/8/1N6/8/1N2KN2 w - - 0 1")
            .unwrap();
        let movement = move_to(
            &rules,
            imported.timeline.current(),
            imported.timeline.history(),
            Position::new(1, 0),
            Position::new(3, 1),
        );
        assert_eq!(
            rules
                .san_for_move(
                    imported.timeline.current(),
                    imported.timeline.history(),
                    movement,
                    None,
                )
                .unwrap(),
            "Nb1d2"
        );
    }

    #[test]
    fn pgn_mainline_import_ignores_comments_nags_and_variations() {
        let rules = ChessRules::standard();
        let game = rules
            .from_pgn(
                "[Event \"Example\"]\n\n1. e4 {main line} e5 2. Nf3 $1 Nc6 (2... Nf6 {sideline ) comment} 3. d4) 3. Bb5 a6 *",
            )
            .unwrap();
        assert_eq!(
            rules.to_fen(game.timeline.current(), game.timeline.history()).unwrap(),
            "r1bqkbnr/1ppp1ppp/p1n5/1B2p3/4P3/5N2/PPPP1PPP/RNBQK2R w KQkq - 0 4"
        );
        let exported = rules.to_pgn(&game.initial_fen, game.timeline.history()).unwrap();
        assert!(exported.contains("1. e4 e5 2. Nf3 Nc6 3. Bb5 a6 *"));
    }

    #[test]
    fn pgn_fen_setup_roundtrips_black_to_move() {
        let rules = ChessRules::standard();
        let pgn = "[SetUp \"1\"]\n[FEN \"4k3/8/8/8/8/8/4P3/4K3 b - - 0 17\"]\n\n17... Kf7 *";
        let game = rules.from_pgn(pgn).unwrap();
        let exported = rules.to_pgn(&game.initial_fen, game.timeline.history()).unwrap();
        assert!(exported.contains("[SetUp \"1\"]"));
        assert!(exported.contains("[FEN \"4k3/8/8/8/8/8/4P3/4K3 b - - 0 17\"]"));
        assert!(exported.contains("17... Kf7 *"));
    }
    #[test]
    fn pgn_result_is_preserved_when_board_does_not_encode_the_reason() {
        let rules = ChessRules::standard();
        let game = rules
            .from_pgn("[Result \"0-1\"]\n\n1. e4 e5 2. Nf3 Nc6 0-1")
            .unwrap();
        assert_eq!(
            rules.status(game.timeline.current(), game.timeline.history()).unwrap().outcome,
            Some(ChessOutcome::RecordedWin {
                winner: crate::BLACK_PLAYER,
                loser: crate::WHITE_PLAYER,
            })
        );
        let exported = rules.to_pgn(&game.initial_fen, game.timeline.history()).unwrap();
        assert!(exported.contains("[Result \"0-1\"]"));
        assert!(exported.trim_end().ends_with("0-1"));
    }

    #[test]
    fn pgn_rejects_conflicting_declared_result() {
        let rules = ChessRules::standard();
        assert!(rules
            .from_pgn("[Result \"1-0\"]\n\n1. e4 e5 0-1")
            .is_err());
    }

    #[test]
    fn pgn_rejects_explicit_unfinished_result_after_checkmate() {
        let rules = ChessRules::standard();
        assert!(rules
            .from_pgn("[Result \"*\"]\n\n1. f3 e5 2. g4 Qh4# *")
            .is_err());
        assert!(rules.from_pgn("1. f3 e5 2. g4 Qh4#").is_ok());
    }

    #[test]
    fn pgn_export_preserves_source_tags_but_recomputes_result() {
        let rules = ChessRules::standard();
        let game = rules
            .from_pgn(
                "[Event \"Club Championship\"]\n[Site \"Prague\"]\n[White \"Alpha\"]\n[Black \"Beta\"]\n[Annotator \"Monumla test\"]\n[Result \"0-1\"]\n\n1. e4 e5 0-1",
            )
            .unwrap();
        let exported = rules
            .to_pgn_with_tags(&game.initial_fen, game.timeline.history(), &game.tags)
            .unwrap();
        assert!(exported.contains("[Event \"Club Championship\"]"));
        assert!(exported.contains("[Site \"Prague\"]"));
        assert!(exported.contains("[White \"Alpha\"]"));
        assert!(exported.contains("[Black \"Beta\"]"));
        assert!(exported.contains("[Annotator \"Monumla test\"]"));
        assert!(exported.contains("[Result \"0-1\"]"));
    }

    #[test]
    fn san_parser_accepts_common_annotations_and_optional_check_suffix() {
        let rules = ChessRules::standard();
        let game = rules.from_pgn("1. f3 e5 2. g4 Qh4$1").unwrap();
        assert!(matches!(
            rules.status(game.timeline.current(), game.timeline.history()).unwrap().outcome,
            Some(ChessOutcome::Checkmate { .. })
        ));
    }

}
