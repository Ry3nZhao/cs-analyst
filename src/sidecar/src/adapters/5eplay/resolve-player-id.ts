const ID_TRANSFER_URL =
  'https://gate.5eplay.com/userinterface/http/v1/userinterface/idTransfer';

/** 5EPlay API uuid (32 hex, optional dashes) */
const UUID_LIKE = /^[0-9a-f]{8}-?[0-9a-f]{4}-?[0-9a-f]{4}-?[0-9a-f]{4}-?[0-9a-f]{12}$/i;

type IdTransferResponse = {
  data?: { uuid?: string };
};

export function is5EPlayUuidLike(input: string): boolean {
  return UUID_LIKE.test(input.trim());
}

async function fetchPlayerIdFromDomain(domain: string): Promise<string> {
  const response = await fetch(ID_TRANSFER_URL, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ trans: { domain } }),
  });

  if (response.status === 400) {
    throw new Error('无效的 5EPlay ID');
  }
  if (!response.ok) {
    throw new Error(`5EPlay idTransfer 失败: HTTP ${response.status}`);
  }

  const data = (await response.json()) as IdTransferResponse;
  const uuid = data.data?.uuid?.trim() ?? '';
  if (!uuid) {
    throw new Error('5EPlay 玩家不存在');
  }
  return uuid;
}

/** Resolve domain (boketto) or UUID to canonical player uuid. */
export async function resolve5EPlayPlayerId(input: string): Promise<string> {
  const trimmed = input.trim();
  if (!trimmed) throw new Error('5EPlay ID 不能为空');
  if (is5EPlayUuidLike(trimmed)) return trimmed.toLowerCase();
  return fetchPlayerIdFromDomain(trimmed);
}
