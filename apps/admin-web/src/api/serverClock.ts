/**
 * 応答の Date ヘッダーから、サーバーの時計と端末の時計のずれ（ミリ秒。サーバー − 端末）を出す。
 * 端末の時刻は送った時刻と受け取った時刻の中間とみなす。Date がない・読めないときは undefined（呼ぶ側は端末の時計のまま）。
 * Date は秒までなので、サーバーの時刻を切り捨てた側に寄せる（開始時刻より前に押せるほうへはずらさない）。
 */
export function clockOffsetMs(dateHeader: string | null | undefined, sentAt: number, receivedAt: number): number | undefined {
  if (!dateHeader) return undefined;
  const serverTime = Date.parse(dateHeader);
  if (Number.isNaN(serverTime)) return undefined;
  return serverTime - (sentAt + receivedAt) / 2;
}
