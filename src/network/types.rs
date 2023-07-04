use either::Either;
use libp2p::relay::inbound::hop;
use libp2p::relay::outbound::stop;
use libp2p::swarm::StreamUpgradeError;
use void::Void;

pub type ComposedErr = Either<
    Either<
        Either<Either<Either<Void, std::io::Error>, std::io::Error>, Void>,
        Either<StreamUpgradeError<Either<hop::FatalUpgradeError, stop::FatalUpgradeError>>, Void>,
    >,
    Void,
>;
