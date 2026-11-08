import type { GameService, gameSummary } from '../../../server/games';
import type { lobby, personal, searchUsers } from '../../../server/queries';
export type LiveGame = Awaited<ReturnType<GameService['state']>>;
export type GameSummary = ReturnType<typeof gameSummary>;
export type Lobby = Awaited<ReturnType<typeof lobby>> & { online: number };
export type Personal = Awaited<ReturnType<typeof personal>> & { online?: string[] };
export type PlayerResults = Awaited<ReturnType<typeof searchUsers>>;
