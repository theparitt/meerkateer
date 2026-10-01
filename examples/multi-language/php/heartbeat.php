<?php

declare(strict_types=1);

require_once __DIR__ . '/bootstrap.php';

use Meerkateer\Client;

$client = Client::fromEnv();
$client->deploy('demo-1', 'local', 'finished');
signal($client, 'ok');
$interval = filter_var(getenv('DEMO_HEARTBEAT_SECONDS') ?: '5', FILTER_VALIDATE_INT);
if (!is_int($interval) || $interval < 1) {
    throw new InvalidArgumentException('DEMO_HEARTBEAT_SECONDS must be a positive integer');
}
while (true) {
    sleep($interval);
    try {
        $client->heartbeat(readState(), LANGUAGE . ' demo heartbeat');
    } catch (Throwable $error) {
        fwrite(STDERR, 'heartbeat delivery failed: ' . $error->getMessage() . PHP_EOL);
    }
}
