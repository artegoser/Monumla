import { lobby } from '../../server/queries';
export async function load() {
	return { lobby: { ...(await lobby()), online: 0 } };
}
