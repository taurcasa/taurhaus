/**
 * Pulled-row bridge geometry.
 *
 * The drawer device renders the selected row in the main panel's own surface
 * material ("pulled"), flush to the rail's right edge. This module computes
 * the geometry that lets that material actually continue across the teal
 * frame gutter into the main panel — the way the titlebar's manila tab
 * connects to the panel — instead of stopping at the rail with scoops that
 * only imply the connection.
 *
 * Why this is computed for elements OUTSIDE the rail: the row lives in the
 * rail's vertical scroll container, and CSS cannot combine `overflow-y: auto`
 * with `overflow-x: visible` (the visible axis computes to auto), so nothing
 * inside the rail can paint across the gutter. The bridge is therefore a
 * clip-wrapper fixed over the gutter column with a material strip sliding
 * inside it:
 *
 *   - The wrapper is sized to the list viewport, so `overflow: hidden` clips
 *     the strip at exactly the same lines the scroll container clips the row
 *     — honest clipping at the extremes with no per-edge special cases.
 *   - The strip is positioned at the row's CONTENT offset (scroll-invariant)
 *     and translated by -scrollTop, so during a scroll only the translation
 *     changes. Where the engine supports scroll-driven animations the
 *     translation runs on the compositor (exact sync with the row by
 *     construction); otherwise the scroll handler writes it in the same
 *     frame.
 *
 * The row's own position is measured from the DOM rather than derived from
 * the virtualizer's offsets: those assume a fixed 36px project row while a
 * row with a branch line renders 50px, so the rect is the only exact source.
 *
 * All inputs are plain rect/scalar data so the math stays testable without a
 * layout engine.
 */

/** The rail scoops' radius: the strip extends this far beyond the row so its
 * edges align with the scoop flare tips and the silhouette stays continuous.
 * (The panel-side fillet radius is 6 — tangent-exact across the 6px gutter,
 * on the house 6/8/999 grid — and lives in the .sidebar-bridge-scoop CSS.) */
export const BRIDGE_FLARE_PX = 8

/** The rail's 1px border, covered by the bridge so no hairline survives. */
export const RAIL_BORDER_PX = 1

/** How far the bridge reaches past the panel's edge to cover the panel's
 * own 1px border plus its 1px inset ring — the hairline pair that would
 * otherwise cross the neck. The cover is painted opaque (see the token in
 * app.css): the panel surface is translucent, and layering it twice
 * brightens light mode visibly. */
export const BRIDGE_PANEL_COVER_PX = 2

/** Fallback gutter width when the main panel cannot be measured — the Shell
 * body's `gap-1.5`. */
export const BRIDGE_PANEL_GAP_PX = 6

/** Widest scrollbar lane the bridge understands. A pulled row stopping short
 * of the rail edge by more than this is geometry we don't recognize, and the
 * material must not float in the gutter beside empty rail. */
export const BRIDGE_MAX_LANE_PX = 12

/** Shortfalls up to this are treated as flush (fractional-DPR rounding). */
const FLUSH_TOLERANCE_PX = 0.5

const INACTIVE = Object.freeze({
  active: false,
  wrapper: null,
  strip: null,
  lane: null,
  scrollRange: 0,
})

/**
 * Compute the bridge frame for one measurement pass.
 *
 * @param {object} input
 * @param {DOMRect|null} input.rowRect   the pulled row (`.sidebar-row-pulled`), or null when nothing is pulled
 * @param {DOMRect|null} input.railRect  the sidebar `<aside>`
 * @param {DOMRect|null} input.listRect  the rail's scroll container (the clip box)
 * @param {number|null}  input.panelLeft measured left edge of the main panel, or null to fall back to the gap token
 * @param {number}       input.scrollTop     list scroll offset at measure time
 * @param {number}       input.scrollHeight  list content height
 * @param {number}       input.clientHeight  list viewport height
 * @returns {{active: boolean, wrapper: ?object, strip: ?object, lane: ?object, scrollRange: number}}
 *   wrapper/strip in viewport coordinates (strip top relative to the wrapper,
 *   stated at scrollTop 0); lane in rail coordinates for the in-rail cover.
 */
export function bridgeFrame({
  rowRect,
  railRect,
  listRect,
  panelLeft = null,
  scrollTop = 0,
  scrollHeight = 0,
  clientHeight = 0,
}) {
  if (!rowRect || !railRect || !listRect) return INACTIVE
  if (listRect.height <= 0) return INACTIVE

  const railInnerRight = railRect.right - RAIL_BORDER_PX
  const laneWidth = railInnerRight - rowRect.right
  // Overhang or a shortfall wider than a scrollbar lane: refuse to bridge.
  if (laneWidth < -FLUSH_TOLERANCE_PX || laneWidth > BRIDGE_MAX_LANE_PX) return INACTIVE

  const resolvedPanelLeft = panelLeft ?? railRect.right + BRIDGE_PANEL_GAP_PX
  if (resolvedPanelLeft <= railRect.right) return INACTIVE

  const strip = {
    // Content coordinates relative to the wrapper: scrolling only changes the
    // -scrollTop translation, never this base.
    top: rowRect.top - listRect.top + scrollTop - BRIDGE_FLARE_PX,
    height: rowRect.height + 2 * BRIDGE_FLARE_PX,
  }

  // The lane element is absolutely positioned inside the rail, whose 1px
  // border makes its PADDING box the containing block — so these offsets
  // are stated relative to the padding-box origin (border-box + border), or
  // the cover lands 1px off on both axes and a rail hairline survives at
  // the row's right edge on space-taking-scrollbar engines.
  const lane = laneWidth > FLUSH_TOLERANCE_PX
    ? {
        left: rowRect.right - (railRect.left + RAIL_BORDER_PX),
        width: laneWidth,
        top: listRect.top - (railRect.top + RAIL_BORDER_PX),
        height: listRect.height,
        strip,
      }
    : null

  return {
    active: true,
    wrapper: {
      left: railInnerRight,
      // Reaches past the panel edge so the strip's opaque cover can blank
      // the panel's border + inset ring along the material's contact span.
      width: resolvedPanelLeft + BRIDGE_PANEL_COVER_PX - railInnerRight,
      top: listRect.top,
      height: listRect.height,
    },
    strip,
    lane,
    scrollRange: Math.max(0, scrollHeight - clientHeight),
  }
}

let scrollTimelineOverride = null

/**
 * Whether the engine can run the strip's -scrollTop translation on the
 * compositor via a scroll-driven animation. When it can't, the scroll handler
 * writes the transform instead.
 *
 * Both properties are required: the strips are not descendants of the
 * scroller, so the timeline only reaches them through `timeline-scope` on
 * the rail. An engine with `animation-timeline` but no `timeline-scope`
 * would apply an animation whose timeline never resolves — while the JS
 * fallback stood down — leaving a frozen strip beside the wrong row. The
 * `@supports` block in app.css states the same two-property condition, so
 * CSS and JS agree by construction.
 */
export function supportsScrollDrivenTracking() {
  if (scrollTimelineOverride !== null) return scrollTimelineOverride
  return typeof CSS !== 'undefined'
    && typeof CSS.supports === 'function'
    && CSS.supports('animation-timeline', '--rail-scroll')
    && CSS.supports('timeline-scope', '--rail-scroll')
}

/** Test hook: force the tracking mode (`true`/`false`), or `null` to detect. */
export function setScrollDrivenTrackingForTesting(value) {
  scrollTimelineOverride = value
}
