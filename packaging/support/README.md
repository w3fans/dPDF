# Public dPDF support pages

These snapshots implement the owner-requested idz.si/bs/dpdf-donate.html page using
the existing RUWAC design and bank-helper interface. The canonical website source
is idz.si's legacy/bs directory; publish through that project's standard build and
release flow. Desktop staging does not install or execute these PHP files.

The payment adapter fixes its dPDF reference and return URLs server-side, validates
amount/contact input and uses a separate DPDF order namespace/private state folder.
It calls the site's existing dirpay helper/configuration; no bank credentials or
configuration are copied here. The return adapter reuses that helper's verification
before reporting a bank-confirmed result. Automated receipts/invoices remain
unavailable, as stated on the page.

Syntax checking and scripts/check-support.php use synthetic fields and a fixture
signing helper. They do not submit a payment, qualify the provider configuration or
establish a completed card transaction. Production smoke checks use only GET:
landing page 200, payment endpoint 405, unverified return 400. Existing RUWAC and
GSE support pages/handlers stay unchanged.
