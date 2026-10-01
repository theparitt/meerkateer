<?php

declare(strict_types=1);

require_once __DIR__ . '/../../../sdk/php/Meerkateer.php';

use Meerkateer\Client;

const LANGUAGE = 'php';

function stateFile(): string
{
    return getenv('DEMO_STATE_FILE') ?: '/tmp/meerkateer-php-demo-state';
}

function readState(): string
{
    $state = @file_get_contents(stateFile());
    return in_array($state, ['ok', 'degraded', 'down'], true) ? $state : 'ok';
}

function writeState(string $state): void
{
    if (file_put_contents(stateFile(), $state, LOCK_EX) === false) {
        throw new RuntimeException('could not persist demo state');
    }
}

function signal(Client $client, string $next): void
{
    $messages = [
        'ok' => 'demo service recovered',
        'degraded' => 'demo dependency is slow',
        'down' => 'demo process is unavailable',
    ];
    $levels = ['ok' => 'info', 'degraded' => 'warning', 'down' => 'error'];
    $kinds = ['ok' => 'demo_recovered', 'degraded' => 'demo_degraded', 'down' => 'demo_failed'];
    $previous = readState();
    writeState($next);
    $client->heartbeat($next, $messages[$next]);
    if ($previous !== $next) {
        $client->event($kinds[$next], $levels[$next], $messages[$next]);
    }
}
