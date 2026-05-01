use crate::board::{PieceType, PlayerColor};
use crate::evaluation::base;
use crate::game::GameState;

const OUTSIDE_PASSED_PAWN_BONUS: [i32; 7] = [240, 140, 75, 40, 20, 10, 0];

// Escort bonus for a piece within Chebyshev distance of an outside-lane pawn
const ESCORT_DIST_BONUS: [i32; 7] = [55, 42, 28, 14, 6, 2, 0];

// Bishop diagonal cover: bonus when bishop's diagonal passes through a pawn's forward square
const BISHOP_DIAG_COVER: i32 = 35;

// Knight mobility tables (no board-edge restriction — Obstocean is infinite)
const KNIGHT_MOB_MG: [i32; 9] = [-62, -36, -12, 0, 8, 14, 18, 20, 22];
const KNIGHT_MOB_EG: [i32; 9] = [-81, -46, -26, -8, 4, 10, 14, 16, 18];

// Flat leaper superiority: in Obstocean, obstacles block sliders; leapers jump freely.
// Applied per-piece on top of PSQT, scales toward EG.
const KNIGHT_MG_SUPERIORITY: i32 = 5;
const KNIGHT_EG_SUPERIORITY: i32 = 40;

// Outside pawn connectivity
const OUTSIDE_PHALANX_BONUS: i32 = 28; // two outside pawns same rank, adjacent files
const OUTSIDE_CHAIN_BONUS: i32 = 18;   // outside pawn diagonally supported

// Phase increments
const PHASE_KNIGHT: i32 = 1;
const PHASE_BISHOP: i32 = 1;
const PHASE_ROOK: i32 = 2;
const PHASE_QUEEN: i32 = 4;

// ─── PIECE-SQUARE TABLES ────────────────────────────────────────────────────
// Indexed [rank_from_home][file_0..7]  (rank 0 = own home rank, rank 7 = promo rank)
// Applied only to pieces inside the 8×8 core (x: 1–8, y: 1–8).
// White: rank_idx = y−1.  Black: rank_idx = 8−y  (mirrors table).

// Knights are the dominant piece — they leap over obstacles freely.
// MG: strong center outposts.  EG: edge files bonus to escort outside pawns.
const KNIGHT_MG_PSQT: [[i32; 8]; 8] = [
    [-30, -20, -15, -15, -15, -15, -20, -30], // rank 1
    [-20,   0,   5,   5,   5,   5,   0, -20],
    [-15,   5,  20,  25,  25,  20,   5, -15],
    [-15,  10,  25,  30,  30,  25,  10, -15],
    [-15,  10,  25,  30,  30,  25,  10, -15],
    [-15,   5,  20,  25,  25,  20,   5, -15],
    [ -5,   5,  15,  20,  20,  15,   5,  -5], // rank 7
    [-30, -20, -15, -15, -15, -15, -20, -30], // rank 8 (promo)
];
const KNIGHT_EG_PSQT: [[i32; 8]; 8] = [
    [-20, -15,   0,   5,   5,   0, -15, -20],
    [-15,   5,  15,  15,  15,  15,   5, -15],
    [  0,  15,  25,  30,  30,  25,  15,   0],
    [  5,  15,  30,  40,  40,  30,  15,   5],
    [  5,  15,  30,  40,  40,  30,  15,   5],
    [  0,  15,  25,  30,  30,  25,  15,   0],
    [ 10,  20,  25,  25,  25,  25,  20,  10],
    [-20, -15,   0,   5,   5,   0, -15, -20],
];

// Bishops: long diagonals covering outside-lane files are prime.
const BISHOP_MG_PSQT: [[i32; 8]; 8] = [
    [-20, -10, -10, -10, -10, -10, -10, -20],
    [-10,   0,   0,   0,   0,   0,   0, -10],
    [-10,   0,  10,  10,  10,  10,   0, -10],
    [-10,   5,  10,  10,  10,  10,   5, -10],
    [-10,   0,  10,  10,  10,  10,   0, -10],
    [-10,   5,   5,  10,  10,   5,   5, -10],
    [-10,   5,   0,   0,   0,   0,   5, -10],
    [-20, -10, -10, -10, -10, -10, -10, -20],
];
const BISHOP_EG_PSQT: [[i32; 8]; 8] = [
    [-10, -10, -10, -10, -10, -10, -10, -10],
    [-10,   0,   0,   0,   0,   0,   0, -10],
    [-10,   0,   5,   5,   5,   5,   0, -10],
    [-10,   0,   5,  10,  10,   5,   0, -10],
    [-10,   0,   5,  10,  10,   5,   0, -10],
    [-10,   0,   5,   5,   5,   5,   0, -10],
    [-10,   0,   0,   0,   0,   0,   0, -10],
    [-10, -10, -10, -10, -10, -10, -10, -10],
];

// Rooks: 7th-rank penetration is decisive; edge files good for racing.
const ROOK_MG_PSQT: [[i32; 8]; 8] = [
    [  0,   0,   5,   5,   5,   5,   0,   0],
    [ -5,   0,   0,   0,   0,   0,   0,  -5],
    [ -5,   0,   0,   0,   0,   0,   0,  -5],
    [ -5,   0,   0,   0,   0,   0,   0,  -5],
    [ -5,   0,   0,   0,   0,   0,   0,  -5],
    [ -5,   0,   0,   0,   0,   0,   0,  -5],
    [  5,  10,  10,  10,  10,  10,  10,   5], // rank 7
    [  0,   0,   0,   5,   5,   0,   0,   0],
];
const ROOK_EG_PSQT: [[i32; 8]; 8] = [
    [  0,   0,   0,   0,   0,   0,   0,   0],
    [  0,   0,   0,   0,   0,   0,   0,   0],
    [  0,   0,   0,   0,   0,   0,   0,   0],
    [  0,   0,   0,   0,   0,   0,   0,   0],
    [  0,   0,   0,   0,   0,   0,   0,   0],
    [  0,   0,   0,   0,   0,   0,   0,   0],
    [ 10,  10,  10,  10,  10,  10,  10,  10], // rank 7
    [  0,   0,   0,   0,   0,   0,   0,   0],
];

// Queens: central activity; not penalised for early development (no knights to develop).
const QUEEN_MG_PSQT: [[i32; 8]; 8] = [
    [-20, -10, -10,  -5,  -5, -10, -10, -20],
    [-10,   0,   0,   0,   0,   0,   0, -10],
    [-10,   0,   5,   5,   5,   5,   0, -10],
    [ -5,   0,   5,   5,   5,   5,   0,  -5],
    [  0,   0,   5,   5,   5,   5,   0,  -5],
    [-10,   5,   5,   5,   5,   5,   0, -10],
    [-10,   0,   5,   0,   0,   0,   0, -10],
    [-20, -10, -10,  -5,  -5, -10, -10, -20],
];
const QUEEN_EG_PSQT: [[i32; 8]; 8] = [
    [-30, -20, -10,   0,   0, -10, -20, -30],
    [-20, -10,   0,   0,   0,   0, -10, -20],
    [-10,   0,  10,  10,  10,  10,   0, -10],
    [  0,   0,  10,  20,  20,  10,   0,   0],
    [  0,   0,  10,  20,  20,  10,   0,   0],
    [-10,   0,  10,  10,  10,  10,   0, -10],
    [-20, -10,   0,   0,   0,   0, -10, -20],
    [-30, -20, -10,   0,   0, -10, -20, -30],
];

// King MG: hide in the corner, away from enemy sliders.
// King EG: march toward edge files and up toward the outside-lane pawn race.
const KING_MG_PSQT: [[i32; 8]; 8] = [
    [ 20,  30,  10,   0,   0,  10,  30,  20], // rank 1 (home)
    [ 20,  20,   0,   0,   0,   0,  20,  20],
    [-10, -20, -20, -20, -20, -20, -20, -10],
    [-20, -30, -30, -40, -40, -30, -30, -20],
    [-30, -40, -40, -50, -50, -40, -40, -30],
    [-30, -40, -40, -50, -50, -40, -40, -30],
    [-30, -40, -40, -50, -50, -40, -40, -30],
    [-30, -40, -40, -50, -50, -40, -40, -30],
];
const KING_EG_PSQT: [[i32; 8]; 8] = [
    [-50, -30, -30, -20, -20, -30, -30, -50], // home rank: terrible
    [-30, -10,   0,  10,  10,   0, -10, -30],
    [-20,   0,  15,  20,  20,  15,   0, -20],
    [-10,  10,  20,  30,  30,  20,  10, -10],
    [-10,  10,  20,  30,  30,  20,  10, -10],
    [  0,  20,  20,  20,  20,  20,  20,   0], // edge files pull king outward
    [ 25,  30,  15,  15,  15,  15,  30,  25], // near outside lane
    [ 30,  20,  10,   5,   5,  10,  20,  30], // at promo rank, edges best
];

// ─── HELPERS ────────────────────────────────────────────────────────────────

/// Tapered PSQT for a piece anywhere on the Obstocean board (bounds: x −6..=15, y −3..=12).
///
/// Core 8×8 (x 1..=8, y 1..=8): detailed rank/file tables.
/// Outside lanes (within board bounds but outside core): flat piece-type bonus + advancement.
/// Out of bounds: 0.
#[inline]
fn psqt_value(pt: PieceType, x: i64, y: i64, color: PlayerColor, phase: i32) -> i32 {
    if !(-6..=15).contains(&x) || !(-3..=12).contains(&y) {
        return 0;
    }

    if (1..=8).contains(&x) && (1..=8).contains(&y) {
        // Core 8×8: detailed positional tables
        let fi = (x - 1) as usize;
        let ri = if color == PlayerColor::White {
            (y - 1) as usize          // y=1 → 0 (home), y=8 → 7 (promo)
        } else {
            (8 - y) as usize           // y=8 → 0 (home), y=1 → 7 (promo)
        };
        let (mg, eg) = match pt {
            PieceType::Knight
            | PieceType::Archbishop
            | PieceType::Centaur
            | PieceType::RoyalCentaur => (KNIGHT_MG_PSQT[ri][fi], KNIGHT_EG_PSQT[ri][fi]),
            PieceType::Bishop => (BISHOP_MG_PSQT[ri][fi], BISHOP_EG_PSQT[ri][fi]),
            PieceType::Rook | PieceType::Chancellor => (ROOK_MG_PSQT[ri][fi], ROOK_EG_PSQT[ri][fi]),
            PieceType::Queen | PieceType::Amazon | PieceType::RoyalQueen => {
                (QUEEN_MG_PSQT[ri][fi], QUEEN_EG_PSQT[ri][fi])
            }
            PieceType::King => (KING_MG_PSQT[ri][fi], KING_EG_PSQT[ri][fi]),
            _ => return 0,
        };
        return (mg * phase + eg * (base::MAX_PHASE - phase)) / base::MAX_PHASE;
    }

    // Outside-lane positions: piece has broken through the obstacle wall.
    // Reward activity there; advancement toward own promo rank matters.
    let promo_rank: i64 = if color == PlayerColor::White { 8 } else { 1 };
    let adv_dist = if color == PlayerColor::White {
        (promo_rank - y).max(0)
    } else {
        (y - promo_rank).max(0)
    };
    let adv = (8 - adv_dist.min(8)) as i32 * 3; // up to +24

    let (mg_base, eg_base) = match pt {
        // Leapers dominate in the obstacle maze — big bonus for being active outside
        PieceType::Knight
        | PieceType::Archbishop
        | PieceType::Centaur
        | PieceType::RoyalCentaur => (8, 22),
        PieceType::Bishop => (4, 10),
        PieceType::Rook | PieceType::Chancellor => (10, 15),
        PieceType::Queen | PieceType::Amazon | PieceType::RoyalQueen => (8, 12),
        // King should march to the edge in EG to support/stop pawn races
        PieceType::King => (0, 22),
        _ => return 0,
    };

    let base_v = (mg_base * phase + eg_base * (base::MAX_PHASE - phase)) / base::MAX_PHASE;
    base_v + adv
}

/// Knight mobility on infinite board (no 8×8 boundary restriction).
#[inline]
fn count_knight_mobility(board: &crate::board::Board, x: i64, y: i64, piece: crate::board::Piece) -> i32 {
    let our_color = piece.color();
    let mut count = 0i32;
    for (dx, dy) in [(2,1),(2,-1),(-2,1),(-2,-1),(1,2),(1,-2),(-1,2),(-1,-2)] {
        match board.get_piece(x + dx, y + dy) {
            None => count += 1,
            Some(p) if p.color() != our_color && p.color() != PlayerColor::Neutral => count += 1,
            _ => {}
        }
    }
    count
}

/// Bonus for a piece being close (Chebyshev) to an outside-lane pawn, scaled by advancement.
#[inline]
fn piece_pawn_escort(px: i64, py: i64, my_pawns: &[(i64, i64)], promo_rank: i64, is_white: bool) -> i32 {
    let mut bonus = 0i32;
    for &(qx, qy) in my_pawns {
        if qx >= 1 && qx <= 8 {
            continue;
        }
        let dist = (px - qx).unsigned_abs().max((py - qy).unsigned_abs()) as usize;
        if dist >= ESCORT_DIST_BONUS.len() {
            continue;
        }
        let advance_dist = if is_white { (promo_rank - qy).max(0) } else { (qy - promo_rank).max(0) };
        let scale = (8 - advance_dist.min(8)) as i32;
        bonus += (ESCORT_DIST_BONUS[dist] * scale) / 8;
    }
    bonus
}

/// Bishop escort + diagonal cover bonus.
#[inline]
fn bishop_pawn_support(bx: i64, by: i64, color: PlayerColor, my_pawns: &[(i64, i64)], promo_rank: i64) -> i32 {
    let forward: i64 = if color == PlayerColor::White { 1 } else { -1 };
    let is_white = color == PlayerColor::White;
    let mut bonus = 0i32;
    for &(px, py) in my_pawns {
        if px >= 1 && px <= 8 {
            continue;
        }
        let dist = (bx - px).unsigned_abs().max((by - py).unsigned_abs()) as usize;
        if dist < ESCORT_DIST_BONUS.len() {
            let advance_dist = if is_white { (promo_rank - py).max(0) } else { (py - promo_rank).max(0) };
            let scale = (8 - advance_dist.min(8)) as i32;
            bonus += (ESCORT_DIST_BONUS[dist] * scale) / 8;
        }
        let fwd_x = px;
        let fwd_y = py + forward;
        let dx = (bx - fwd_x).abs();
        let dy = (by - fwd_y).abs();
        if dx == dy && dx > 0 {
            bonus += BISHOP_DIAG_COVER;
        }
    }
    bonus
}

/// Knight evaluation: mobility + leaper superiority taper + escort + PSQT.
#[inline]
fn eval_knight(
    game: &GameState,
    x: i64,
    y: i64,
    piece: crate::board::Piece,
    phase: i32,
    my_pawns: &[(i64, i64)],
    promo_rank: i64,
) -> i32 {
    let mob = count_knight_mobility(&game.board, x, y, piece);
    let mob_idx = (mob as usize).min(8);
    let mg_mob = KNIGHT_MOB_MG[mob_idx];
    let eg_mob = KNIGHT_MOB_EG[mob_idx];
    let mob_bonus = (mg_mob * phase + eg_mob * (base::MAX_PHASE - phase)) / base::MAX_PHASE;
    let superiority = (KNIGHT_MG_SUPERIORITY * phase + KNIGHT_EG_SUPERIORITY * (base::MAX_PHASE - phase))
        / base::MAX_PHASE;
    mob_bonus + superiority + piece_pawn_escort(x, y, my_pawns, promo_rank, piece.color() == PlayerColor::White)
}

/// Outside pawn connectivity: phalanx and chain bonuses.
#[inline]
fn eval_outside_pawn_structure(pawns: &[(i64, i64)]) -> i32 {
    let mut bonus = 0i32;
    for i in 0..pawns.len() {
        let (ax, ay) = pawns[i];
        if ax >= 1 && ax <= 8 {
            continue;
        }
        for j in (i + 1)..pawns.len() {
            let (bx, by) = pawns[j];
            if bx >= 1 && bx <= 8 {
                continue;
            }
            let dx = (ax - bx).abs();
            let dy = (ay - by).abs();
            if dx == 1 {
                if dy == 0 {
                    bonus += OUTSIDE_PHALANX_BONUS;
                } else if dy == 1 {
                    bonus += OUTSIDE_CHAIN_BONUS;
                }
            }
        }
    }
    bonus
}

// ─── PAWN EVAL ───────────────────────────────────────────────────────────────

/// Lane-based pawn evaluation: huge outside bonus, edge priority, center penalty.
#[inline]
fn eval_pawn(x: i64, y: i64, color: PlayerColor, game: &GameState) -> i32 {
    let dist = if color == PlayerColor::White {
        (game.white_promo_rank - y).max(0)
    } else {
        (y - game.black_promo_rank).max(0)
    };

    let mut b: i32 = 0;
    b += (8 - dist.min(8)) as i32 * 10; // advancement

    if x < 1 {
        b += 100 + ((1 - x) as i32 * 20);
        b += OUTSIDE_PASSED_PAWN_BONUS[(dist as usize).min(6)];
    } else if x > 8 {
        b += 100 + ((x - 8) as i32 * 20);
        b += OUTSIDE_PASSED_PAWN_BONUS[(dist as usize).min(6)];
    } else if x == 1 || x == 8 {
        b += 75;
    } else if x == 2 || x == 7 {
        b += 25;
    } else {
        b -= 40; // center penalty
    }

    b
}

// ─── RACE EVAL ───────────────────────────────────────────────────────────────

/// Promotion race: who's closest on the outside/edge lanes?
fn race_eval_optimized(game: &GameState, white_pawns: &[(i64, i64)], black_pawns: &[(i64, i64)]) -> i32 {
    let mut w_min: i64 = 100;
    let mut b_min: i64 = 100;

    for &(x, y) in white_pawns.iter() {
        if x > 1 && x < 8 {
            continue;
        }
        let d = (game.white_promo_rank - y).max(0);
        if d < w_min {
            w_min = d;
        }
    }
    for &(x, y) in black_pawns.iter() {
        if x > 1 && x < 8 {
            continue;
        }
        let d = (y - game.black_promo_rank).max(0);
        if d < b_min {
            b_min = d;
        }
    }

    let mut s: i32 = 0;
    if w_min < 100 && b_min < 100 {
        let diff = b_min - w_min;
        s += (diff as i32 * 100).clamp(-500, 500);
    } else if w_min < 100 {
        s += (10 - w_min).max(0) as i32 * 40;
    } else if b_min < 100 {
        s -= (10 - b_min).max(0) as i32 * 40;
    }
    s
}

// ─── MAIN EVALUATOR ──────────────────────────────────────────────────────────

#[inline]
pub fn evaluate(game: &GameState) -> i32 {
    evaluate_inner(game)
}

#[inline]
fn evaluate_inner(game: &GameState) -> i32 {
    let mut score = game.material_score;
    let white_royals = game.white_royals.as_slice();
    let black_royals = game.black_royals.as_slice();

    base::EVAL_WHITE_PAWNS.with(|wp_cell| {
        base::EVAL_BLACK_PAWNS.with(|bp_cell| {
            base::EVAL_PIECE_LIST.with(|pl_cell| {
                base::EVAL_WHITE_RQ.with(|wrq_cell| {
                    base::EVAL_BLACK_RQ.with(|brq_cell| {
                        let white_pawns = unsafe { &mut *wp_cell.get() };
                        let black_pawns = unsafe { &mut *bp_cell.get() };
                        let heavy_pieces = unsafe { &mut *pl_cell.get() };
                        let white_rq = unsafe { &mut *wrq_cell.get() };
                        let black_rq = unsafe { &mut *brq_cell.get() };

                        white_pawns.clear();
                        black_pawns.clear();
                        heavy_pieces.clear();
                        white_rq.clear();
                        black_rq.clear();

                        let mut phase: i32 = 0;

                        // 1. Board scan: collect pieces + accumulate phase
                        for (cx, cy, tile) in game.board.tiles.iter() {
                            if crate::simd::both_zero(tile.occ_white, tile.occ_black) {
                                continue;
                            }
                            let mut bits = tile.occ_all;
                            while bits != 0 {
                                let idx = bits.trailing_zeros() as usize;
                                bits &= bits - 1;
                                let packed = tile.piece[idx];
                                if packed == 0 {
                                    continue;
                                }
                                let p = crate::board::Piece::from_packed(packed);
                                if p.color() == PlayerColor::Neutral {
                                    continue;
                                }
                                let pt = p.piece_type();
                                let x = cx * 8 + (idx % 8) as i64;
                                let y = cy * 8 + (idx / 8) as i64;

                                phase += match pt {
                                    PieceType::Knight
                                    | PieceType::Archbishop
                                    | PieceType::Centaur
                                    | PieceType::RoyalCentaur => PHASE_KNIGHT,
                                    PieceType::Bishop => PHASE_BISHOP,
                                    PieceType::Rook | PieceType::Chancellor => PHASE_ROOK,
                                    PieceType::Queen
                                    | PieceType::Amazon
                                    | PieceType::RoyalQueen => PHASE_QUEEN,
                                    _ => 0,
                                };

                                if pt == PieceType::Pawn {
                                    let v = eval_pawn(x, y, p.color(), game);
                                    if p.color() == PlayerColor::White {
                                        score += v;
                                        white_pawns.push((x, y));
                                    } else {
                                        score -= v;
                                        black_pawns.push((x, y));
                                    }
                                } else {
                                    heavy_pieces.push((x, y, p));
                                    match pt {
                                        PieceType::Rook
                                        | PieceType::Queen
                                        | PieceType::Amazon
                                        | PieceType::Chancellor
                                        | PieceType::RoyalQueen => {
                                            if p.color() == PlayerColor::White {
                                                white_rq.push((x, y));
                                            } else {
                                                black_rq.push((x, y));
                                            }
                                        }
                                        _ => {}
                                    }
                                }
                            }
                        }

                        let phase = phase.min(base::MAX_PHASE);

                        // 2. Sort pawn lists for O(log N) file lookups
                        white_pawns.sort_unstable();
                        black_pawns.sort_unstable();

                        // 3. Non-pawn piece evaluation
                        for &(x, y, p) in heavy_pieces.iter() {
                            let is_white = p.color() == PlayerColor::White;
                            let pt = p.piece_type();
                            let my_pawns: &[(i64, i64)] = if is_white { white_pawns } else { black_pawns };
                            let promo_rank = if is_white { game.white_promo_rank } else { game.black_promo_rank };

                            let functional = match pt {
                                PieceType::Knight
                                | PieceType::Archbishop
                                | PieceType::Centaur
                                | PieceType::RoyalCentaur => {
                                    eval_knight(game, x, y, p, phase, my_pawns, promo_rank)
                                }
                                PieceType::Bishop => {
                                    base::evaluate_bishop(
                                        game, x, y, p.color(),
                                        white_royals, black_royals,
                                        phase, white_pawns, black_pawns,
                                    ) + bishop_pawn_support(x, y, p.color(), my_pawns, promo_rank)
                                }
                                PieceType::Rook | PieceType::Chancellor | PieceType::Amazon => {
                                    base::evaluate_rook(
                                        game, x, y, p.color(),
                                        white_royals, black_royals,
                                        phase, white_pawns, black_pawns,
                                    ) + piece_pawn_escort(x, y, my_pawns, promo_rank, is_white)
                                }
                                PieceType::Queen | PieceType::RoyalQueen => {
                                    base::evaluate_queen(
                                        game, x, y, p.color(),
                                        white_royals, black_royals,
                                        phase, white_pawns, black_pawns,
                                    ) + piece_pawn_escort(x, y, my_pawns, promo_rank, is_white)
                                }
                                _ => 0, // King: PSQT only
                            };

                            let positional = psqt_value(pt, x, y, p.color(), phase);
                            let v = functional + positional;

                            if is_white { score += v; } else { score -= v; }
                        }

                        // 4. Pawn structure
                        score += base::evaluate_pawn_structure_traced(
                            game,
                            phase,
                            white_royals,
                            black_royals,
                            &mut base::NoTrace,
                            white_pawns,
                            black_pawns,
                            white_rq,
                            black_rq,
                        );

                        // 5. Outside pawn connectivity
                        score += eval_outside_pawn_structure(white_pawns);
                        score -= eval_outside_pawn_structure(black_pawns);

                        // 6. Promotion race
                        score += race_eval_optimized(game, white_pawns, black_pawns);
                    });
                });
            });
        });
    });

    if game.turn == PlayerColor::Black { -score } else { score }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::GameState;

    fn create_obstocean_game() -> GameState {
        let mut game = GameState::new();
        game.variant = Some(crate::Variant::Obstocean);
        game.white_promo_rank = 8;
        game.black_promo_rank = 1;
        game
    }

    fn create_obstocean_game_from_icn(icn: &str) -> GameState {
        let mut game = create_obstocean_game();
        game.setup_position_from_icn(icn);
        game
    }

    #[test]
    fn test_evaluate_returns_value() {
        let mut game = create_obstocean_game_from_icn("w (8;q|1;q) K5,1|k5,8");
        game.turn = PlayerColor::White;
        game.recompute_hash();
        let score = evaluate(&game);
        assert!(score.abs() < 10000, "K vs K should be near 0");
    }

    #[test]
    fn test_edge_pawn_bonus() {
        let mut game = create_obstocean_game_from_icn("w (8;q|1;q) K5,1|k5,8");
        game.white_promo_rank = 8;
        let edge = eval_pawn(1, 4, PlayerColor::White, &game);
        let center = eval_pawn(4, 4, PlayerColor::White, &game);
        assert!(edge > center, "Edge pawn ({}) > center pawn ({})", edge, center);
    }

    #[test]
    fn test_eval_pawn_function() {
        let mut game = create_obstocean_game_from_icn("w (8;q|1;q) K5,1|k5,8");
        game.white_promo_rank = 8;
        let edge = eval_pawn(1, 3, PlayerColor::White, &game);
        let center = eval_pawn(4, 3, PlayerColor::White, &game);
        assert!(edge > center, "Edge pawn should score higher");
    }

    #[test]
    fn test_race_eval_basic() {
        let mut game = create_obstocean_game_from_icn("w (8;q|1;q) K5,1|k5,8|P1,7");
        game.white_promo_rank = 8;
        game.black_promo_rank = 1;
        let mut w = Vec::new();
        let mut b = Vec::new();
        for (x, y, p) in game.board.iter() {
            if p.piece_type() == PieceType::Pawn {
                if p.color() == PlayerColor::White { w.push((x, y)); }
                else if p.color() == PlayerColor::Black { b.push((x, y)); }
            }
        }
        assert!(race_eval_optimized(&game, &w, &b) > 0, "White near promo should be positive");
    }

    #[test]
    fn test_outside_file_bonus() {
        let mut game = create_obstocean_game_from_icn("w (8;q|1;q) K5,1|k5,8");
        game.white_promo_rank = 8;
        let outside = eval_pawn(0, 4, PlayerColor::White, &game);
        let edge = eval_pawn(1, 4, PlayerColor::White, &game);
        assert!(outside > edge, "Outside pawn should score best");
    }

    #[test]
    fn test_race_eval_both_sides() {
        let mut game = create_obstocean_game_from_icn("w (8;q|1;q) K5,1|k5,8|P1,7|p1,4");
        game.white_promo_rank = 8;
        game.black_promo_rank = 1;
        let mut w = Vec::new();
        let mut b = Vec::new();
        for (x, y, p) in game.board.iter() {
            if p.piece_type() == PieceType::Pawn {
                if p.color() == PlayerColor::White { w.push((x, y)); }
                else if p.color() == PlayerColor::Black { b.push((x, y)); }
            }
        }
        assert!(race_eval_optimized(&game, &w, &b) > 0, "White closer to promo: {}", race_eval_optimized(&game, &w, &b));
    }

    #[test]
    fn test_evaluate_inner_returns_value() {
        let mut game = create_obstocean_game_from_icn("w (8;q|1;q) K5,1|k5,8|P4,4|p3,5");
        game.white_promo_rank = 8;
        game.black_promo_rank = 1;
        let score = evaluate_inner(&game);
        assert!(score.abs() < 100000, "Score should be reasonable: {}", score);
    }

    #[test]
    fn test_black_advantage_race() {
        let mut game = create_obstocean_game_from_icn("w (8;q|1;q) K5,1|k5,8|P1,3|p1,2");
        game.white_promo_rank = 8;
        game.black_promo_rank = 1;
        let mut w = Vec::new();
        let mut b = Vec::new();
        for (x, y, p) in game.board.iter() {
            if p.piece_type() == PieceType::Pawn {
                if p.color() == PlayerColor::White { w.push((x, y)); }
                else if p.color() == PlayerColor::Black { b.push((x, y)); }
            }
        }
        assert!(race_eval_optimized(&game, &w, &b) < 0, "Black closer to promo: {}", race_eval_optimized(&game, &w, &b));
    }

    #[test]
    fn test_bishop_escorts_pawn() {
        let mut game = create_obstocean_game_from_icn("w (8;q|1;q) K5,1|k5,8");
        game.white_promo_rank = 8;
        let close = bishop_pawn_support(0, 5, PlayerColor::White, &[(0, 6)], 8);
        let far = bishop_pawn_support(3, 3, PlayerColor::White, &[(0, 6)], 8);
        assert!(close > far, "Closer bishop should get more escort bonus");
        assert!(close > 0, "Bishop near outside pawn should get bonus");
    }

    #[test]
    fn test_knight_mobility_bonus() {
        let game = create_obstocean_game_from_icn("w (8;q|1;q) K5,1|k5,8|N0,4");
        for (x, y, p) in game.board.iter() {
            if p.piece_type() == PieceType::Knight && p.color() == PlayerColor::White {
                let mob = count_knight_mobility(&game.board, x, y, p);
                assert!(mob > 0, "Knight should have mobility");
                let score = eval_knight(&game, x, y, p, base::MAX_PHASE / 2, &[], game.white_promo_rank);
                assert!(score.abs() < 500, "Knight eval should be reasonable");
                break;
            }
        }
    }

    #[test]
    fn test_outside_pawn_phalanx() {
        let pawns = vec![(-1i64, 5i64), (-2i64, 5i64)];
        let bonus = eval_outside_pawn_structure(&pawns);
        assert!(bonus >= OUTSIDE_PHALANX_BONUS, "Phalanx should give bonus: {}", bonus);
    }

    #[test]
    fn test_outside_pawn_chain() {
        let pawns = vec![(-1i64, 5i64), (-2i64, 4i64)];
        let bonus = eval_outside_pawn_structure(&pawns);
        assert!(bonus >= OUTSIDE_CHAIN_BONUS, "Chain should give bonus: {}", bonus);
    }

    #[test]
    fn test_psqt_knight_center_beats_corner() {
        // Knight in center of 8x8 should score better than corner (MG)
        let center = psqt_value(PieceType::Knight, 4, 4, PlayerColor::White, base::MAX_PHASE);
        let corner = psqt_value(PieceType::Knight, 1, 1, PlayerColor::White, base::MAX_PHASE);
        assert!(center > corner, "Knight center PSQT {} > corner {}", center, corner);
    }

    #[test]
    fn test_psqt_king_eg_advances() {
        // King at rank 6 (y=6) should score better than at home (y=1) in EG
        let advanced = psqt_value(PieceType::King, 1, 6, PlayerColor::White, 0);
        let home = psqt_value(PieceType::King, 5, 1, PlayerColor::White, 0);
        assert!(advanced > home, "King EG advanced {} > home {}", advanced, home);
    }

    #[test]
    fn test_psqt_outside_lane_nonzero() {
        // Pieces in the outside lane (within board bounds) get a positive PSQT
        let lane = psqt_value(PieceType::Knight, 0, 4, PlayerColor::White, 0); // EG
        assert!(lane > 0, "Knight in outside lane should get positive PSQT: {}", lane);
        // Truly out-of-bounds still returns 0
        let oob = psqt_value(PieceType::Knight, -7, 4, PlayerColor::White, 0);
        assert_eq!(oob, 0, "Out-of-bounds should return 0");
    }

    #[test]
    fn test_psqt_black_mirrors_white() {
        // Black piece at y=8 (home) should match White piece at y=1 (home) for king MG
        let w = psqt_value(PieceType::King, 1, 1, PlayerColor::White, base::MAX_PHASE);
        let b = psqt_value(PieceType::King, 1, 8, PlayerColor::Black, base::MAX_PHASE);
        assert_eq!(w, b, "Black/White home PSQT should mirror: w={} b={}", w, b);
    }
}
