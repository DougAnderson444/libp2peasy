use either::Either;
use libp2p::relay::inbound::hop;
use libp2p::relay::outbound::stop;
use libp2p::swarm::ConnectionHandlerUpgrErr;
use tokio::io;
use void::Void;

pub type ComposedErr = Either<
    Either<
        Either<Either<Either<Void, io::Error>, io::Error>, Void>,
        Either<
            ConnectionHandlerUpgrErr<Either<hop::FatalUpgradeError, stop::FatalUpgradeError>>,
            Void,
        >,
    >,
    ConnectionHandlerUpgrErr<io::Error>,
>;
