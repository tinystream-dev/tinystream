// SPDX-License-Identifier: AGPL-3.0-or-later

type Tinted = { backdrop?: string | null; poster?: string | null; backdropTint?: string | null; posterTint?: string | null }

/** The colour a title tints its page with: its backdrop's, else its poster's. */
export function titleTint(t: Tinted | null | undefined): string | null {
  return (t?.backdrop ? t.backdropTint : t?.posterTint) ?? null
}

/** For `refetchInterval`: asks again, a few times, while the server is still working a tint out. */
export function awaitTint<T>(pending: (data: T) => boolean) {
  return (query: { state: { data: T | undefined; dataUpdateCount: number } }) =>
    query.state.data !== undefined && query.state.dataUpdateCount < 4 && pending(query.state.data) ? 2000 : false
}

/** Whether a title has artwork for its tint that hasn't been tinted yet. */
export function tintPending(t: Tinted | null | undefined): boolean {
  return !!t && (t.backdrop ? !t.backdropTint : !!t.poster && !t.posterTint)
}
