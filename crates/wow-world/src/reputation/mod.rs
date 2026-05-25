//! C++ `game/Reputation` runtime state.

pub mod mgr;

pub use mgr::{
    FactionStateLikeCpp, ForcedReactionsLikeCpp, RepListIdLikeCpp, ReputationMgrLikeCpp,
    ReputationRankCountersLikeCpp, reputation_to_rank_like_cpp,
};
