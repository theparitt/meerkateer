# Meerkateer PHP SDK

The PHP 8.2+ SDK sends MKS-1 heartbeat, event, and deployment facts using built-in PHP
streams. It requires HTTPS except for loopback development, rejects redirects, bounds
payloads and responses, and retries transient failures with one idempotency key.

Copy `sdk/php/Meerkateer.php` into your application, then set the service environment
values shown by the Console:

```php
<?php
require_once __DIR__ . '/Meerkateer.php';

use Meerkateer\Client;

$watch = Client::fromEnv();
$watch->heartbeat('ok', 'worker ready');
$watch->event('queue_delay', 'warning', 'jobs delayed');
```

For WordPress or WooCommerce integrations, call the SDK from a bounded task or hook
you control. This package does not inspect orders or modify store data. Never put
customer data, order contents, credentials, or raw exception traces in messages.
