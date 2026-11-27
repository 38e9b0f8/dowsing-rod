package payments;

// Stripe payment provider implementation.
public class StripeGateway implements PaymentGateway {
    private final StripeClient stripeClient;
    private final AuditLog auditLog;

    public StripeGateway(StripeClient stripeClient, AuditLog auditLog) {
        this.stripeClient = stripeClient;
        this.auditLog = auditLog;
    }

    @Override
    public ChargeResult charge(long amount, String currency, String customerId) {
        long validated = Validation.validateAmount(amount);
        ChargeRequest request = Requests.buildChargeRequest(validated, currency, customerId);
        ProviderResponse response = stripeClient.charges().create(request);
        if (!"succeeded".equals(response.getStatus())) {
            throw new PaymentException("Stripe charge failed: " + response.getFailureMessage());
        }
        ChargeResult result = Results.normalizeChargeResult(response);
        auditLog.record("charge", "stripe", result);
        return result;
    }

    @Override
    public RefundResult refund(String transactionId, Long amount) {
        Transaction transaction = Transactions.lookup(transactionId);
        long refundAmount = amount != null ? amount : transaction.getAmount();
        RefundRequest request = Requests.buildRefundRequest(transaction.getProviderId(), refundAmount);
        ProviderResponse response = stripeClient.refunds().create(request);
        if (!"succeeded".equals(response.getStatus())) {
            throw new PaymentException("Stripe refund failed: " + response.getFailureMessage());
        }
        RefundResult result = Results.normalizeRefundResult(response);
        auditLog.record("refund", "stripe", result);
        return result;
    }
}
