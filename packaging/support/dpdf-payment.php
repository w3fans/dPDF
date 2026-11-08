<?php
declare(strict_types=1);

// Reuse the existing server-side payment configuration and verified bank helpers.
require_once dirname(__DIR__) . '/dirpay/lib.php';

function dpdf_order_path(string $oid): string {
    if (!preg_match('/^DPDF-[a-f0-9]{24}$/D', $oid)) { throw new InvalidArgumentException('Invalid order'); }
    return dirname(__DIR__) . '/private/dpdf/' . $oid . '.json';
}

function dpdf_payment_fields(array $input, array $settings): array {
    foreach (['amount', 'name', 'email'] as $key) {
        if (isset($input[$key]) && !is_string($input[$key])) { throw new InvalidArgumentException('Invalid contribution'); }
    }
    $amount = str_replace(',', '.', trim($input['amount'] ?? ''));
    if (!preg_match('/^(?:0|[1-9][0-9]{0,4})(?:\.[0-9]{1,2})?$/D', $amount)) {
        throw new InvalidArgumentException('Enter a valid contribution amount');
    }
    $parts = explode('.', $amount);
    $cents = ((int)$parts[0]) * 100 + (int)str_pad($parts[1] ?? '', 2, '0');
    if ($cents < 100 || $cents > 1000000) { throw new InvalidArgumentException('Contribution must be between 1 and 10000 EUR'); }
    $name = trim($input['name'] ?? ''); $email = trim($input['email'] ?? '');
    if (strlen($name) > 160 || strlen($email) > 254 || preg_match('/[\x00-\x1F\x7F]/', $name . $email)) {
        throw new InvalidArgumentException('Invalid contact details');
    }
    if ($email !== '' && !filter_var($email, FILTER_VALIDATE_EMAIL)) { throw new InvalidArgumentException('Enter a valid optional email'); }
    $fields = ['clientid' => $settings['client_id'], 'oid' => 'DPDF-' . bin2hex(random_bytes(12)),
        'amount' => sprintf('%d.%02d', intdiv($cents, 100), $cents % 100),
        'okurl' => 'https://idz.si/bs/dpdf-return.php', 'failUrl' => 'https://idz.si/bs/dpdf-return.php',
        'TranType' => 'Auth', 'rnd' => bin2hex(random_bytes(16)), 'currency' => '978',
        'storetype' => '3d_pay_hosting', 'lang' => 'en', 'encoding' => 'utf-8', 'hashAlgorithm' => 'ver2',
        'cardName' => $name, 'cltEmail' => $email, 'orderRef' => 'dPDF development support'];
    $fields['hash'] = dirpay_hash_ver2($fields, $settings['store_key']);
    return $fields;
}

if (defined('DPDF_PAYMENT_TEST')) { return; }
header('Cache-Control: no-store, private');
header('Referrer-Policy: no-referrer');
header('X-Content-Type-Options: nosniff');
if ($_SERVER['REQUEST_METHOD'] !== 'POST') {
    header('Allow: POST'); http_response_code(405); exit('Use the dPDF support page.');
}
$origin = $_SERVER['HTTP_ORIGIN'] ?? '';
if ($origin !== '' && $origin !== 'https://idz.si') { http_response_code(403); exit('Invalid request origin.'); }
try {
    $settings = dirpay_config();
    if ($settings['store_key'] === '' || $settings['bank_url'] !== 'https://activa.eway2pay.com/fim/est3Dgate') { throw new RuntimeException('Bank unavailable'); }
    $fields = dpdf_payment_fields($_POST, $settings);
    $path = dpdf_order_path($fields['oid']);
    if (!is_dir(dirname($path)) && !mkdir(dirname($path), 0700, true) && !is_dir(dirname($path))) { throw new RuntimeException('Order state unavailable'); }
    $state = ['oid'=>$fields['oid'], 'amount'=>$fields['amount'], 'currency'=>'978', 'client_id'=>$fields['clientid'], 'created_at'=>time()];
    $oldMask = umask(0077);
    try { $file = fopen($path, 'x'); if ($file === false) { throw new RuntimeException('Order state unavailable'); } try { if (fwrite($file, json_encode($state, JSON_THROW_ON_ERROR)) === false) { throw new RuntimeException('Order state unavailable'); } } finally { fclose($file); } } finally { umask($oldMask); }

} catch (InvalidArgumentException $error) {
    http_response_code(400); exit(htmlspecialchars($error->getMessage(), ENT_QUOTES, 'UTF-8'));
} catch (Throwable $error) {
    http_response_code(503); exit('Payment is temporarily unavailable. Please try again later.');
}
$nonce = base64_encode(random_bytes(24));
header("Content-Security-Policy: default-src 'none'; script-src 'nonce-$nonce'; form-action https://activa.eway2pay.com; base-uri 'none'; frame-ancestors 'none'");
header('Content-Type: text/html; charset=utf-8');
?>
<!doctype html><html lang="en"><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>Opening secure bank payment</title>
<body><p>Opening secure bank payment…</p>
<form id="bank-payment" method="post" action="https://activa.eway2pay.com/fim/est3Dgate">
<?php foreach ($fields as $name => $value): ?>
<input type="hidden" name="<?= htmlspecialchars($name, ENT_QUOTES, 'UTF-8') ?>" value="<?= htmlspecialchars($value, ENT_QUOTES, 'UTF-8') ?>">
<?php endforeach; ?>
<noscript><button type="submit">Continue to secure bank payment</button></noscript>
</form><script nonce="<?= htmlspecialchars($nonce, ENT_QUOTES, 'UTF-8') ?>">document.getElementById('bank-payment').submit();</script></body></html>
