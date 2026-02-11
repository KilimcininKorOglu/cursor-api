/// Stripe subscription status enum
///
/// Subscription lifecycle states defined based on Stripe API
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, ::rkyv::Archive, ::rkyv::Serialize, ::rkyv::Deserialize,
)]
#[repr(u8)]
pub enum SubscriptionStatus {
    /// Trial period - customer can safely use the product, automatically transitions to active after first payment
    Trialing,

    /// Active state - subscription is in good standing, can provide service normally
    Active,

    /// Incomplete - customer must successfully pay within 23 hours to activate subscription
    /// Or payment requires additional action (such as customer authentication)
    Incomplete,

    /// Incomplete and expired - initial payment failed and no successful payment within 23 hours
    /// These subscriptions will not be charged to the customer, used to track customers with failed activation
    IncompleteExpired,

    /// Past due - latest invoice payment failed or payment not attempted
    /// Subscription continues to generate invoices, can transition to canceled/unpaid or remain past_due based on settings
    PastDue,

    /// Already canceled - subscription is already canceled, terminal state, cannot be updated
    /// Automatic collection of unpaid invoices during cancellation is disabled
    Canceled,

    /// Unpaid - latest invoice is unpaid but subscription still exists
    /// Invoice remains open and continues to be generated, but no payment is attempted
    /// Product access should be revoked because payment has been attempted and retried during past_due period
    Unpaid,

    /// Already paused - trial period ended but no default payment method and set to pause
    /// No longer creates invoices for subscription, can be resumed after adding payment method
    Paused,
}

impl SubscriptionStatus {
    const TRIALING: &'static str = "trialing";
    const ACTIVE: &'static str = "active";
    const INCOMPLETE: &'static str = "incomplete";
    const INCOMPLETE_EXPIRED: &'static str = "incomplete_expired";
    const PAST_DUE: &'static str = "past_due";
    const CANCELED: &'static str = "canceled";
    const UNPAID: &'static str = "unpaid";
    const PAUSED: &'static str = "paused";

    #[inline]
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            Self::TRIALING => Some(Self::Trialing),
            Self::ACTIVE => Some(Self::Active),
            Self::INCOMPLETE => Some(Self::Incomplete),
            Self::INCOMPLETE_EXPIRED => Some(Self::IncompleteExpired),
            Self::PAST_DUE => Some(Self::PastDue),
            Self::CANCELED => Some(Self::Canceled),
            Self::UNPAID => Some(Self::Unpaid),
            Self::PAUSED => Some(Self::Paused),
            _ => None,
        }
    }

    #[inline]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Trialing => Self::TRIALING,
            Self::Active => Self::ACTIVE,
            Self::Incomplete => Self::INCOMPLETE,
            Self::IncompleteExpired => Self::INCOMPLETE_EXPIRED,
            Self::PastDue => Self::PAST_DUE,
            Self::Canceled => Self::CANCELED,
            Self::Unpaid => Self::UNPAID,
            Self::Paused => Self::PAUSED,
        }
    }
}

impl ::serde::Serialize for SubscriptionStatus {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where S: ::serde::Serializer {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> ::serde::Deserialize<'de> for SubscriptionStatus {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where D: ::serde::Deserializer<'de> {
        let s = <String as ::serde::Deserialize>::deserialize(deserializer)?;
        Self::from_str(&s).ok_or_else(|| {
            ::serde::de::Error::custom(format_args!("unknown subscription status: {s}"))
        })
    }
}
