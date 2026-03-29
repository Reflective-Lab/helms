#![allow(dead_code)]

pub mod prio {
    pub mod common {
        pub mod v1 {
            tonic::include_proto!("prio.common.v1");
        }
    }

    pub mod conversations {
        pub mod v1 {
            tonic::include_proto!("prio.conversations.v1");
        }
    }

    pub mod documents {
        pub mod v1 {
            tonic::include_proto!("prio.documents.v1");
        }
    }

    pub mod facts {
        pub mod v1 {
            tonic::include_proto!("prio.facts.v1");
        }
    }

    pub mod identity {
        pub mod v1 {
            tonic::include_proto!("prio.identity.v1");
        }
    }

    pub mod metadata {
        pub mod v1 {
            tonic::include_proto!("prio.metadata.v1");
        }
    }

    pub mod modules {
        pub mod v1 {
            tonic::include_proto!("prio.modules.v1");
        }
    }

    pub mod opportunities {
        pub mod v1 {
            tonic::include_proto!("prio.opportunities.v1");
        }
    }

    pub mod parties {
        pub mod v1 {
            tonic::include_proto!("prio.parties.v1");
        }
    }

    pub mod workflow {
        pub mod v1 {
            tonic::include_proto!("prio.workflow.v1");
        }
    }

    pub mod truths {
        pub mod v1 {
            tonic::include_proto!("prio.truths.v1");
        }
    }
}

pub use prio::common::v1 as common;
pub use prio::conversations::v1 as conversations;
pub use prio::documents::v1 as documents;
pub use prio::facts::v1 as facts;
pub use prio::identity::v1 as identity;
pub use prio::metadata::v1 as metadata;
pub use prio::modules::v1 as modules;
pub use prio::opportunities::v1 as opportunities;
pub use prio::parties::v1 as parties;
pub use prio::truths::v1 as truths;
pub use prio::workflow::v1 as workflow;
