<?php
declare(strict_types=1);
define('DPDF_PAYMENT_TEST', true);
require_once __DIR__ . '/dpdf-payment.php';
header('Cache-Control: no-store, private');
header('Referrer-Policy: no-referrer');
header("Content-Security-Policy: default-src 'none'; base-uri 'none'; frame-ancestors 'none'");
header('Content-Type: text/html; charset=utf-8');
$message = 'Payment could not be verified. No confirmation or invoice has been issued.';
$status = 400;
try {
    if ($_SERVER['REQUEST_METHOD'] !== 'POST') { throw new InvalidArgumentException('POST required'); }
    $oid = dirpay_post_value($_POST, 'oid');
    if ($oid === '') { $oid = dirpay_post_value($_POST, 'ReturnOid'); }
    $path = dpdf_order_path($oid);
    if (!is_file($path) || is_link($path) || filesize($path) > 4096) { throw new RuntimeException('Unknown order'); }
    $state = json_decode(file_get_contents($path), true, 16, JSON_THROW_ON_ERROR);
    if (($state['created_at'] ?? 0) < time() - 172800) { throw new RuntimeException('Expired order'); }
    [$valid, $reason] = dirpay_verify_bank_response($_POST, $state);
    if (!$valid) { throw new RuntimeException('Invalid bank response'); }
    $approved = dirpay_post_value($_POST, 'ProcReturnCode') === '00' && strcasecmp(dirpay_post_value($_POST, 'Response'), 'Approved') === 0;
    $message = $approved ? 'The bank confirmed your dPDF contribution. Thank you for supporting development by Denis Zvegelj. Automated email confirmation and invoices are not available yet.' : 'The bank did not approve this payment. You may return to the support page and try again.';
    $status = 200;
} catch (Throwable $error) { /* Never expose provider payloads or contact data. */ }
http_response_code($status);
?><!doctype html><html lang="en"><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>dPDF payment result</title><body><h1>dPDF support</h1><p><?= htmlspecialchars($message, ENT_QUOTES, 'UTF-8') ?></p><p><a href="https://idz.si/bs/dpdf-donate.html">Return to support page</a></p></body></html>
