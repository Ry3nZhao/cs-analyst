import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { is5EPlayUuidLike, resolve5EPlayPlayerId } from './resolve-player-id.js';

describe('resolve5EPlayPlayerId', () => {
  it('returns uuid-like input without idTransfer', async () => {
    const uuid = 'a1b2c3d4-e5f6-7890-abcd-ef1234567890';
    assert.equal(is5EPlayUuidLike(uuid), true);
    assert.equal(await resolve5EPlayPlayerId(uuid), uuid.toLowerCase());
  });

  it('calls idTransfer for domain', async () => {
    const originalFetch = globalThis.fetch;
    let callCount = 0;
    globalThis.fetch = async () => {
      callCount += 1;
      return new Response(JSON.stringify({ data: { uuid: 'resolved-uuid-1234' } }), { status: 200 });
    };

    try {
      assert.equal(await resolve5EPlayPlayerId('boketto'), 'resolved-uuid-1234');
      assert.equal(callCount, 1);
    } finally {
      globalThis.fetch = originalFetch;
    }
  });
});
