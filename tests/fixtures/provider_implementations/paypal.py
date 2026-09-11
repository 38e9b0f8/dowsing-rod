# PayPal payment provider implementation.


def charge(amount, currency, customer_id):
    """Process a payment via PayPal."""
    validated = validate_amount(amount)
    request = build_charge_request(validated, currency, customer_id)
    response = paypal_client.payments.create(request)
    if response.status != "COMPLETED":
        raise PaymentError(f"PayPal charge failed: {response.error_message}")
    result = normalize_charge_result(response)
    audit_log.record("charge", "paypal", result)
    return result


def refund(transaction_id, amount=None):
    """Issue a refund via PayPal."""
    transaction = lookup_transaction(transaction_id)
    refund_amount = amount or transaction.amount
    request = build_refund_request(transaction.provider_id, refund_amount)
    response = paypal_client.refunds.create(request)
    if response.status != "COMPLETED":
        raise PaymentError(f"PayPal refund failed: {response.error_message}")
    result = normalize_refund_result(response)
    audit_log.record("refund", "paypal", result)
    return result


def get_status(transaction_id):
    """Check transaction status via PayPal."""
    transaction = lookup_transaction(transaction_id)
    response = paypal_client.payments.retrieve(transaction.provider_id)
    status = map_paypal_status(response.status)
    return {"transaction_id": transaction_id, "status": status, "provider": "paypal"}
