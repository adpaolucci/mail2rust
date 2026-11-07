import { EmailMessage, FilterCriteria } from '../types/mail';

/**
 * Pure evaluation engine for message filtering
 */
export function matchesFilter(msg: EmailMessage, criteria: FilterCriteria): boolean {
  // Date filtering
  if (criteria.startDate) {
    const msgDate = new Date(msg.date).getTime();
    const filterStart = new Date(criteria.startDate).getTime();
    if (msgDate < filterStart) return false;
  }

  if (criteria.endDate) {
    const msgDate = new Date(msg.date).getTime();
    // End date should include the full end day up to 23:59:59.999
    const filterEnd = new Date(criteria.endDate).getTime() + (24 * 60 * 60 * 1000 - 1);
    if (msgDate > filterEnd) return false;
  }

  // Sender filtering
  if (criteria.senderSearch && criteria.senderSearch.trim() !== '') {
    const query = criteria.senderSearch.toLowerCase().trim();
    const senderName = msg.from.name.toLowerCase();
    const senderEmail = msg.from.email.toLowerCase();
    if (!senderName.includes(query) && !senderEmail.includes(query)) {
      return false;
    }
  }

  // Recipient filtering
  if (criteria.recipientSearch && criteria.recipientSearch.trim() !== '') {
    const query = criteria.recipientSearch.toLowerCase().trim();
    const hasMatch = msg.to.some(
      r => r.name.toLowerCase().includes(query) || r.email.toLowerCase().includes(query)
    ) || (msg.cc && msg.cc.some(
      r => r.name.toLowerCase().includes(query) || r.email.toLowerCase().includes(query)
    ));
    if (!hasMatch) return false;
  }

  // Subject filtering
  if (criteria.subjectSearch && criteria.subjectSearch.trim() !== '') {
    const query = criteria.subjectSearch.toLowerCase().trim();
    if (!msg.subject.toLowerCase().includes(query)) return false;
  }

  // Attachments filter
  if (criteria.hasAttachments !== undefined && criteria.hasAttachments !== null) {
    const hasAtt = msg.attachments && msg.attachments.length > 0;
    if (criteria.hasAttachments && !hasAtt) return false;
    if (!criteria.hasAttachments && hasAtt) return false;
  }

  // Size filtering
  if (criteria.minSizeBytes !== undefined && criteria.minSizeBytes !== null) {
    if (msg.sizeBytes < criteria.minSizeBytes) return false;
  }

  if (criteria.maxSizeBytes !== undefined && criteria.maxSizeBytes !== null) {
    if (msg.sizeBytes > criteria.maxSizeBytes) return false;
  }

  // Importance
  if (criteria.importance && criteria.importance !== 'all') {
    if (msg.importance !== criteria.importance) return false;
  }

  // Read status
  if (criteria.readStatus && criteria.readStatus !== 'all') {
    if (criteria.readStatus === 'read' && !msg.isRead) return false;
    if (criteria.readStatus === 'unread' && msg.isRead) return false;
  }

  // Flagged
  if (criteria.flaggedOnly && !msg.isFlagged) {
    return false;
  }

  // Legal hold
  if (criteria.legalHoldOnly && !msg.legalHold) {
    return false;
  }

  return true;
}

/**
 * Filter an array of messages
 */
export function filterMessages(messages: EmailMessage[], criteria: FilterCriteria): EmailMessage[] {
  return messages.filter(msg => matchesFilter(msg, criteria));
}
