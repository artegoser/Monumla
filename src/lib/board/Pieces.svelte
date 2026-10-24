<script lang="ts">
	import { tick } from 'svelte';
	import BoardArrow from './BoardArrow.svelte';
	import { movementTrack } from './movement';
	import { stateScalar } from '$lib/wasm/runtime';
	import type {
		ChoiceView,
		EntityView,
		GameView,
		InteractionView,
		PositionView,
		TransitionView
	} from '$lib/wasm/runtime';

	export let game: GameView;
	export let interaction: InteractionView;
	export let previousGame: GameView | null = null;
	export let transition: TransitionView | null = null;
	export let animationSeq = 0;
	export let orientation: 'white' | 'black' = 'white';
	export let onchoice: (choiceId: string) => void;
	export let oncancel: () => void;
	export let onanimationchange: (active: boolean) => void = () => {};

	type DragState = {
		pointerId: number;
		entityId: number;
		origin: PositionView;
		startX: number;
		startY: number;
		x: number;
		y: number;
		started: boolean;
		previouslySelected: boolean;
	};

	type Arrow = { from: PositionView; to: PositionView; color: string };
	type SquareAnnotation = { position: PositionView; color: string };

	let boardElement: HTMLDivElement;
	let drag: DragState | null = null;
	let hover: PositionView | null = null;
	let suppressClick = false;
	let drawStart: PositionView | null = null;
	let drawCurrent: PositionView | null = null;
	let drawColor = '#15781b';
	let arrows: Arrow[] = [];
	let squareAnnotations: SquareAnnotation[] = [];
	let assetOverrides: Record<number, string> = {};
	let fadingPieces: EntityView[] = [];
	let seenAnimationSeq = -1;
	let cleanupTimer: ReturnType<typeof setTimeout> | null = null;
	let activeAnimations: Animation[] = [];
	let seenOrientation: 'white' | 'black' = orientation;
	let animationToken = 0;

	function key(position: PositionView) {
		return `${position.x}:${position.y}`;
	}

	function samePosition(a: PositionView | null | undefined, b: PositionView | null | undefined) {
		return a != null && b != null && a.x === b.x && a.y === b.y;
	}

	function assetPathFromKey(assetKey: string) {
		if (assetKey.startsWith('chess/')) {
			const [, side, kind] = assetKey.split('/');
			const directory = side === 'white' ? 'W' : 'B';
			const filename = kind ? kind[0].toUpperCase() + kind.slice(1) : 'Pawn';
			return `/pieces/${directory}/${filename}.svg`;
		}
		return `/rulesets/${assetKey}.svg`;
	}

	function display(
		position: PositionView,
		orientationValue: 'white' | 'black',
		width: number,
		height: number
	) {
		return orientationValue === 'white'
			? { x: position.x, y: height - position.y - 1 }
			: { x: width - position.x - 1, y: position.y };
	}

	function entityStyle(
		entity: EntityView,
		currentDrag: DragState | null,
		orientationValue: 'white' | 'black',
		width: number,
		height: number,
		board: HTMLDivElement | undefined
	) {
		const shown = display(entity.position, orientationValue, width, height);
		if (currentDrag?.started && currentDrag.entityId === entity.id && board) {
			const bounds = board.getBoundingClientRect();
			const square = bounds.width / width;
			return `left:${currentDrag.x - square / 2}px;top:${currentDrag.y - square / 2}px;width:${square}px;height:${square}px;transform:none;`;
		}
		return `left:${(shown.x * 100) / width}%;top:${(shown.y * 100) / height}%;width:${100 / width}%;height:${100 / height}%;`;
	}

	function fadingStyle(
		entity: EntityView,
		orientationValue: 'white' | 'black',
		width: number,
		height: number
	) {
		const shown = display(entity.position, orientationValue, width, height);
		return `left:${(shown.x * 100) / width}%;top:${(shown.y * 100) / height}%;width:${100 / width}%;height:${100 / height}%;`;
	}

	function squareStyle(
		position: PositionView,
		orientationValue: 'white' | 'black',
		width: number,
		height: number
	) {
		const shown = display(position, orientationValue, width, height);
		return `left:${(shown.x * 100) / width}%;top:${(shown.y * 100) / height}%;width:${100 / width}%;height:${100 / height}%;`;
	}

	$: entitiesBySquare = new Map(game.entities.map((entity) => [key(entity.position), entity]));
	$: entitiesById = new Map(game.entities.map((entity) => [entity.id, entity]));
	$: entityChoices = new Map(
		interaction.choices
			.filter(
				(choice): choice is ChoiceView & { entity: number } =>
					choice.kind === 'select_entity' && choice.entity != null
			)
			.map((choice) => [choice.entity, choice])
	);
	$: positionChoices = interaction.choices.filter(
		(choice): choice is ChoiceView & { position: PositionView } =>
			choice.kind === 'select_position' && choice.position != null
	);
	$: visualActor = drag?.entityId ?? interaction.selected_entity;
	$: visibleDestinations = positionChoices.filter(
		(choice) => choice.actor == null || (visualActor != null && choice.actor === visualActor)
	);
	$: destinationBySquare = new Map(
		visibleDestinations.map((choice) => [key(choice.position), choice])
	);
	$: draggableEntityIds = new Set(
		positionChoices.filter((choice) => choice.actor != null).map((choice) => choice.actor as number)
	);
	$: selectedPosition =
		visualActor == null ? null : (entitiesById.get(visualActor)?.position ?? drag?.origin ?? null);
	$: optionChoices = interaction.choices.filter(
		(choice): choice is ChoiceView & { option_key: string } =>
			choice.kind === 'select_option' && choice.option_key != null
	);

	function choiceForMove(actor: number, position: PositionView) {
		return positionChoices.find(
			(choice) => choice.actor === actor && samePosition(choice.position, position)
		);
	}

	function globalChoiceForPosition(position: PositionView) {
		return positionChoices.find(
			(choice) => choice.actor == null && samePosition(choice.position, position)
		);
	}

	function positionFromPointer(clientX: number, clientY: number): PositionView | null {
		if (!boardElement) return null;
		const bounds = boardElement.getBoundingClientRect();
		if (
			clientX < bounds.left ||
			clientX >= bounds.right ||
			clientY < bounds.top ||
			clientY >= bounds.bottom
		)
			return null;
		let screenX = Math.floor(((clientX - bounds.left) / bounds.width) * game.width);
		let screenY = Math.floor(((clientY - bounds.top) / bounds.height) * game.height);
		if (orientation === 'black') {
			screenX = game.width - screenX - 1;
			return { x: screenX, y: screenY };
		}
		return { x: screenX, y: game.height - screenY - 1 };
	}

	function clearAnnotations() {
		arrows = [];
		squareAnnotations = [];
	}

	function annotationColor(event: PointerEvent) {
		if (event.shiftKey) return '#882020';
		if (event.altKey) return '#003088';
		if (event.ctrlKey || event.metaKey) return '#e68f00';
		return '#15781b';
	}

	function cancelActiveAnimations(clearTransientState = false) {
		onanimationchange(false);
		for (const animation of activeAnimations) animation.cancel();
		activeAnimations = [];
		if (cleanupTimer) {
			clearTimeout(cleanupTimer);
			cleanupTimer = null;
		}
		if (clearTransientState) {
			assetOverrides = {};
			fadingPieces = [];
		}
	}

	function cancelEntityAnimation(entityId: number) {
		if (!boardElement) return;
		const node = boardElement.querySelector<HTMLElement>(
			`.piece-node[data-entity-id="${entityId}"]`
		);
		for (const animation of node?.getAnimations() ?? []) animation.cancel();
	}

	function handlePointerDown(event: PointerEvent, position: PositionView) {
		if (event.button === 2) {
			event.preventDefault();
			drawStart = position;
			drawCurrent = position;
			drawColor = annotationColor(event);
			(event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
			return;
		}
		if (event.button !== 0) return;
		const entity = entitiesBySquare.get(key(position));
		if (!entity || !entityChoices.has(entity.id) || !draggableEntityIds.has(entity.id)) return;
		cancelEntityAnimation(entity.id);
		if (arrows.length || squareAnnotations.length) clearAnnotations();
		drag = {
			pointerId: event.pointerId,
			entityId: entity.id,
			origin: entity.position,
			startX: event.clientX,
			startY: event.clientY,
			x: event.clientX - boardElement.getBoundingClientRect().left,
			y: event.clientY - boardElement.getBoundingClientRect().top,
			started: false,
			previouslySelected: interaction.selected_entity === entity.id
		};
		(event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
	}

	function handleSquareEnter(position: PositionView) {
		if (!drag && destinationBySquare.has(key(position))) hover = position;
	}

	function handleSquareLeave(position: PositionView) {
		if (!drag && samePosition(hover, position)) hover = null;
	}

	function handlePointerMove(event: PointerEvent) {
		if (drawStart) {
			drawCurrent = positionFromPointer(event.clientX, event.clientY);
			return;
		}
		if (!drag || drag.pointerId !== event.pointerId) return;
		const distance = Math.hypot(event.clientX - drag.startX, event.clientY - drag.startY);
		if (!drag.started && distance >= 3) drag = { ...drag, started: true };
		if (!drag.started) return;
		if (event.pointerType === 'touch') event.preventDefault();
		const bounds = boardElement.getBoundingClientRect();
		drag = {
			...drag,
			x: event.clientX - bounds.left,
			y: event.clientY - bounds.top
		};
		const candidate = positionFromPointer(event.clientX, event.clientY);
		hover = candidate && choiceForMove(drag.entityId, candidate) ? candidate : null;
	}

	function finishDrawing(end: PositionView | null) {
		if (!drawStart) return;
		if (end && samePosition(drawStart, end)) {
			const existing = squareAnnotations.findIndex((item) => samePosition(item.position, end));
			if (existing >= 0)
				squareAnnotations = squareAnnotations.filter((_, index) => index !== existing);
			else squareAnnotations = [...squareAnnotations, { position: end, color: drawColor }];
		} else if (end) {
			const existing = arrows.findIndex(
				(item) => samePosition(item.from, drawStart) && samePosition(item.to, end)
			);
			if (existing >= 0) arrows = arrows.filter((_, index) => index !== existing);
			else arrows = [...arrows, { from: drawStart, to: end, color: drawColor }];
		}
		drawStart = null;
		drawCurrent = null;
	}

	function handlePointerUp(event: PointerEvent) {
		if (drawStart) {
			event.preventDefault();
			finishDrawing(positionFromPointer(event.clientX, event.clientY));
			suppressClick = true;
			return;
		}
		if (!drag || drag.pointerId !== event.pointerId) return;
		const finished = drag;
		drag = null;
		hover = null;
		if (!finished.started) return;
		suppressClick = true;
		const destination = positionFromPointer(event.clientX, event.clientY);
		const choice = destination ? choiceForMove(finished.entityId, destination) : undefined;
		if (choice && !samePosition(destination, finished.origin)) onchoice(choice.id);
		else if (finished.previouslySelected) oncancel();
	}

	function handleSquareClick(position: PositionView) {
		if (suppressClick) {
			suppressClick = false;
			return;
		}
		const entity = entitiesBySquare.get(key(position));
		if (entity && entityChoices.has(entity.id)) {
			onchoice(entityChoices.get(entity.id)!.id);
			return;
		}
		if (interaction.selected_entity != null) {
			const choice = choiceForMove(interaction.selected_entity, position);
			if (choice) {
				onchoice(choice.id);
				return;
			}
		}
		const globalChoice = globalChoiceForPosition(position);
		if (globalChoice) onchoice(globalChoice.id);
		else if (interaction.selected_entity != null) oncancel();
	}

	async function startTransitionAnimation() {
		if (seenAnimationSeq === animationSeq) return;
		seenAnimationSeq = animationSeq;

		const token = ++animationToken;
		cancelActiveAnimations();
		assetOverrides = {};
		fadingPieces = [];

		if (!transition || !previousGame || typeof window === 'undefined') return;
		const previousById = new Map(previousGame.entities.map((entity) => [entity.id, entity]));
		const nextById = new Map(game.entities.map((entity) => [entity.id, entity]));
		const paths = new Map<number, PositionView[]>();
		const overrides: Record<number, string> = {};
		const removed: EntityView[] = [];

		for (const change of transition.changes) {
			if (change.type === 'entity_moved') {
				const id = Number(change.entity);
				const from = change.from as PositionView;
				const to = change.to as PositionView;
				const path = paths.get(id) ?? [previousById.get(id)?.position ?? from];
				if (samePosition(path[path.length - 1], from)) {
					path.push(to);
					paths.set(id, path);
				}
			}
			if (change.type === 'entity_removed') {
				const snapshot = change.entity as { id?: number } | undefined;
				const old = snapshot?.id != null ? previousById.get(snapshot.id) : undefined;
				if (old) removed.push(old);
			}
			if (change.type === 'entity_type_changed') {
				const entity = Number(change.entity);
				const old = previousById.get(entity);
				if (old && nextById.has(entity)) overrides[entity] = old.asset_key;
			}
		}

		assetOverrides = overrides;
		fadingPieces = removed;
		onanimationchange(true);
		await tick();
		if (token !== animationToken || !boardElement) return;

		if (window.matchMedia('(prefers-reduced-motion: reduce)').matches) {
			assetOverrides = {};
			fadingPieces = [];
			onanimationchange(false);
			return;
		}

		const hops = Math.max(1, ...Array.from(paths.values(), (path) => path.length - 1));
		const duration = hops > 1 ? hops * 320 : 200;
		const animations: Animation[] = [];
		for (const [entity, path] of paths) {
			const node = boardElement.querySelector<HTMLElement>(
				`.piece-node[data-entity-id="${entity}"]:not(.fading-piece)`
			);
			const final = nextById.get(entity);
			if (!node || !final) continue;
			const track = movementTrack(path, final.position, orientation, game.width, game.height, hops);
			animations.push(node.animate(track.frames, { duration: track.duration, fill: 'backwards' }));
		}
		for (const entity of removed) {
			const node = boardElement.querySelector<HTMLElement>(
				`.fading-piece[data-entity-id="${entity.id}"]`
			);
			if (!node) continue;
			animations.push(
				node.animate([{ opacity: 1 }, { opacity: 0 }], {
					duration: 160,
					delay: Math.max(0, duration - 160),
					fill: 'backwards',
					easing: 'ease-out'
				})
			);
		}
		activeAnimations = animations;

		const cleanup = () => {
			if (token !== animationToken) return;
			activeAnimations = [];
			onanimationchange(false);
			assetOverrides = {};
			fadingPieces = [];
			if (cleanupTimer) {
				clearTimeout(cleanupTimer);
				cleanupTimer = null;
			}
		};

		if (animations.length === 0) {
			cleanup();
			return;
		}
		Promise.allSettled(animations.map((animation) => animation.finished)).then(cleanup);
		cleanupTimer = setTimeout(cleanup, duration + 80);
	}

	$: if (animationSeq !== seenAnimationSeq) void startTransitionAnimation();
	$: if (orientation !== seenOrientation) {
		seenOrientation = orientation;
		animationToken += 1;
		cancelActiveAnimations(true);
	}
</script>

<div
	class="board-layer board-interaction"
	class:go-board-interaction={game.board_style === 'go'}
	class:go-black-to-play={game.board_style === 'go' && game.active_players[0] === 1}
	class:go-white-to-play={game.board_style === 'go' && game.active_players[0] === 2}
	bind:this={boardElement}
	role="group"
	aria-label={`${game.title} interaction surface`}
	style="--width: {game.width}; --height: {game.height};"
	on:pointermove={handlePointerMove}
	on:pointerup={handlePointerUp}
	on:pointercancel={handlePointerUp}
>
	<div class="square-state-layer" aria-hidden="true">
		{#each { length: game.height } as _, y}
			{#each { length: game.width } as _, x}
				{@const position = { x, y }}
				{@const destination = destinationBySquare.get(key(position))}
				{@const occupied = entitiesBySquare.has(key(position))}
				<div
					class="board-state-square"
					class:selected={samePosition(selectedPosition, position)}
					class:last-move={samePosition(game.last_move?.from, position) ||
						samePosition(game.last_move?.to, position)}
					class:check={samePosition(game.status.checked_position, position)}
					class:move-dest={destination != null && !occupied && game.board_style !== 'go'}
					class:capture-dest={destination != null && occupied && game.board_style !== 'go'}
					class:destination-hover={destination != null && samePosition(hover, position)}
					style={squareStyle(position, orientation, game.width, game.height)}
				></div>
			{/each}
		{/each}
	</div>

	<div class="annotation-layer" aria-hidden="true">
		{#each squareAnnotations as annotation}
			<div
				class="annotation-square"
				style={`${squareStyle(annotation.position, orientation, game.width, game.height)}--annotation-color:${annotation.color};`}
			></div>
		{/each}
		<svg viewBox="0 0 100 100" preserveAspectRatio="none">
			{#each arrows as arrow}
				<BoardArrow
					from={arrow.from}
					to={arrow.to}
					{orientation}
					width={game.width}
					height={game.height}
					color={arrow.color}
					thickness={1.5}
				/>
			{/each}
			{#if drawStart && drawCurrent && !samePosition(drawStart, drawCurrent)}
				<g opacity="0.75"
					><BoardArrow
						from={drawStart}
						to={drawCurrent}
						{orientation}
						width={game.width}
						height={game.height}
						color={drawColor}
						thickness={1.5}
					/></g
				>
			{/if}
		</svg>
	</div>

	<div class="piece-layer" aria-hidden="true">
		{#each fadingPieces as entity (entity.id)}
			<img
				class="piece-node fading-piece"
				data-entity-id={entity.id}
				style={fadingStyle(entity, orientation, game.width, game.height)}
				src={assetPathFromKey(entity.asset_key)}
				alt=""
				draggable="false"
			/>
		{/each}
		{#each game.entities as entity (entity.id)}
			{@const hp = stateScalar(entity.presentation_data, 'hp')}
			{@const mana = stateScalar(entity.presentation_data, 'mana')}
			{@const agreedDead = stateScalar(entity.presentation_data, 'agreed_dead') === true}
			{@const disputedDead = stateScalar(entity.presentation_data, 'disputed_dead') === true}
			{#if drag?.started && drag.entityId === entity.id}
				<img
					class="piece-node origin-ghost"
					data-entity-id={entity.id}
					style={fadingStyle(entity, orientation, game.width, game.height)}
					src={assetPathFromKey(assetOverrides[entity.id] ?? entity.asset_key)}
					alt=""
					draggable="false"
				/>
			{/if}
			<img
				class="piece-node"
				data-entity-id={entity.id}
				class:dragging={drag?.started && drag.entityId === entity.id}
				class:go-agreed-dead={agreedDead}
				class:go-disputed-dead={disputedDead}
				style={entityStyle(entity, drag, orientation, game.width, game.height, boardElement)}
				src={assetPathFromKey(assetOverrides[entity.id] ?? entity.asset_key)}
				alt=""
				draggable="false"
			/>
			{#if game.board_style === 'go' && samePosition(game.last_move?.to, entity.position)}
				<svg
					class="piece-node go-last-move"
					style={fadingStyle(entity, orientation, game.width, game.height)}
					viewBox="0 0 100 100"
				>
					<circle
						cx="50"
						cy="50"
						r="10"
						fill="none"
						stroke={entity.asset_key.includes('/white/') ? '#35342f' : '#f4f0e6'}
						stroke-width="3"
					/>
				</svg>
			{/if}
			{#if hp != null}
				<div class="entity-stats" style={fadingStyle(entity, orientation, game.width, game.height)}>
					<span>HP {hp}</span>
					{#if mana != null}<span>MP {mana}</span>{/if}
				</div>
			{/if}
		{/each}
	</div>

	<div class="hit-layer" role="group" aria-label={`${game.title} board`}>
		{#each { length: game.height } as _, row}
			{#each { length: game.width } as _, col}
				{@const x = orientation === 'white' ? col : game.width - col - 1}
				{@const y = orientation === 'white' ? game.height - row - 1 : row}
				{@const position = { x, y }}
				{@const entity = entitiesBySquare.get(key(position))}
				<button
					type="button"
					class="board-hit-square"
					class:movable-origin={entity != null && draggableEntityIds.has(entity.id)}
					class:selectable-entity={entity != null && entityChoices.has(entity.id)}
					class:legal-target={destinationBySquare.has(key(position))}
					aria-label={entity?.label
						? `${entity.label} on ${x + 1},${y + 1}`
						: `Square ${x + 1},${y + 1}`}
					on:pointerenter={() => handleSquareEnter(position)}
					on:pointerleave={() => handleSquareLeave(position)}
					on:contextmenu|preventDefault
					on:pointerdown={(event) => handlePointerDown(event, position)}
					on:click={() => handleSquareClick(position)}
				></button>
			{/each}
		{/each}
	</div>

	{#if optionChoices.some((choice) => choice.asset_key) && interaction.pending_target}
		{@const shown = display(interaction.pending_target, orientation, game.width, game.height)}
		<div
			class="promotion-menu"
			class:from-top={shown.y === 0}
			class:from-bottom={shown.y !== 0}
			style={`left:${(shown.x * 100) / game.width}%;${shown.y === 0 ? 'top:0;' : 'bottom:0;'}width:${100 / game.width}%;`}
			aria-label="Choose option"
		>
			{#each optionChoices.filter((choice) => choice.asset_key) as choice}
				<button
					type="button"
					on:click={() => onchoice(choice.id)}
					aria-label={choice.label ?? choice.option_key}
				>
					{#if choice.asset_key}<img
							src={assetPathFromKey(choice.asset_key)}
							alt={choice.label ?? choice.option_key}
							draggable="false"
						/>{/if}
				</button>
			{/each}
		</div>
	{/if}
</div>
